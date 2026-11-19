// microvm/boot/uefi_main.zig
// Agent-OS Stage-0 UEFI Measured Bootloader (BOOTX64.EFI)
// Target: x86_64-uefi-msvc

const std = @import("std");
const builtin = @import("builtin");
const uefi = std.os.uefi;
const elf = std.elf;
const Sha384 = std.crypto.hash.sha2.Sha384;
const proto = @import("protocol");

// Optional import of generated build-time hash module
const kernel_hash = @import("kernel_hash");
const kernel_payload align(8) = @embedFile("kernel_payload").*;

const L = std.unicode.utf8ToUtf16LeStringLiteral;

// ============================================================================
// 1. PYTHIA ATTESTATION POLICY & CONFIDENTIAL COMPUTING GUIDS
// ============================================================================

/// Set to `true` in production builds to enforce bit-for-bit SHA-384 match
pub const ENFORCE_MEASURED_BOOT: bool = true;

/// Linux/UEFI Confidential Computing SEV-SNP Secrets Table GUID (ade5a61e-13c2-4f9d-b65b-1f61f845a606)
pub const EFI_CC_BLOB_GUID = uefi.Guid{
    .time_low = 0xade5a61e,
    .time_mid = 0x13c2,
    .time_high_and_version = 0x4f9d,
    .clock_seq_high_and_reserved = 0xb6,
    .clock_seq_low = 0x5b,
    .node = .{ 0x1f, 0x61, 0xf8, 0x45, 0xa6, 0x06 },
};

pub const EfiCcBlobTable = extern struct {
    version: u16,
    reserved: u16,
    secrets_paddr: u64,
    secrets_len: u32,
    rsvd1: u32,
    cpuid_paddr: u64,
    cpuid_len: u32,
    rsvd2: u32,
};

// Static 64-bit Page Tables for post-ExitBootServices handoff (0..4 GiB identity mapped + Above-4G GOP/Loader MMIO)
var pml4: [512]u64 align(4096) = @splat(0);
var pdpt: [512]u64 align(4096) = @splat(0);
var pd_0: [512]u64 align(4096) = @splat(0); // 0..1 GiB
var pd_1: [512]u64 align(4096) = @splat(0); // 1..2 GiB
var pd_2: [512]u64 align(4096) = @splat(0); // 2..3 GiB
var pd_3: [512]u64 align(4096) = @splat(0); // 3..4 GiB
var pdpt_fb_hi: [512]u64 align(4096) = @splat(0);
var pd_fb_hi: [512]u64 align(4096) = @splat(0);
var pdpt_ldr_hi: [512]u64 align(4096) = @splat(0);
var pd_ldr_hi: [512]u64 align(4096) = @splat(0);

var static_regions: [proto.MAX_MEMORY_REGIONS]proto.MemoryRegion = undefined;
var boot_info: proto.BootInfo align(64) = proto.BootInfo.initDefault();
var handoff_stack: [64 * 1024]u8 align(16) = undefined;

// ============================================================================
// 2. UEFI ENTRY POINT (Microsoft x64 ABI)
// ============================================================================

pub fn main() uefi.Status {
    const st = uefi.system_table;
    const con = st.con_out orelse return .aborted;
    const bs = st.boot_services orelse return .aborted;

    _ = con.reset(false) catch {};
    puts(con, "\r\n==============================================================\r\n");
    puts(con, "||                  WELL · AGENTOS LOADER                  ||\r\n");
    puts(con, "||               SHA-384 Payload Integrity                 ||\r\n");
    puts(con, "==============================================================\r\n\r\n");

    // 1. Read \EFI\TRISMEGISTUS\KERNEL.ELF into memory pool
    const elf_bytes: []const u8 = &kernel_payload;

    // 2. Compute SHA-384 Measurement over raw ELF image before parsing headers
    Sha384.hash(elf_bytes, &boot_info.kernel_sha384, .{});
    printSha384(con, &boot_info.kernel_sha384);

    if (ENFORCE_MEASURED_BOOT and !std.mem.eql(u8, &boot_info.kernel_sha384, &kernel_hash.EXPECTED_KERNEL_SHA384)) {
        puts(con, "[PYTHIA FATAL] SHA-384 Digest Mismatch! Halting boot.\r\n");
        return .security_violation;
    }

    // 3. Allocate the dedicated 2 MiB VirtIO & HERMES DMA Window at 0x00200000
    var dma_phys: u64 = 0x0020_0000;
    const target_dma_ptr: [*]align(4096) uefi.Page = @ptrFromInt(0x0020_0000);
    if (bs.allocatePages(.{ .address = target_dma_ptr }, .loader_data, 512)) |pages| {
        dma_phys = @intFromPtr(pages.ptr);
    } else |_| {
        // Fallback: allocate anywhere 2 MiB aligned if firmware reserved 0x00200000
        dma_phys = allocate2MibAligned(bs) catch {
            puts(con, "[FATAL] Cannot allocate 2 MiB aligned VirtIO DMA Arena\r\n");
            return .out_of_resources;
        };
    }
    boot_info.dma_pool_phys = dma_phys;
    boot_info.dma_pool_virt = dma_phys;
    @memset(@as([*]u8, @ptrFromInt(dma_phys))[0..0x0020_0000], 0);

    // 4. Parse ELF64 and load PT_LOAD segments into physical memory
    const kernel_entry = loadElf64Segments(bs, con, elf_bytes) catch {
        puts(con, "[FATAL] ELF64 validation or segment allocation failed\r\n");
        return .load_error;
    };

    // 5. Query and Initialize UEFI Graphics Output Protocol (GOP)
    if (initGopFramebuffer(bs, con)) |fb_cfg| {
        boot_info.fb = fb_cfg;
        const shadow_pages = (fb_cfg.buffer_size + 4095) >> 12;
        const max_low_ptr: [*]align(4096) uefi.Page = @ptrFromInt(0xFFFF_F000);
        if (bs.allocatePages(.{ .max_address = max_low_ptr }, .loader_data, shadow_pages)) |pages| {
            boot_info.fb.shadow_base = @intFromPtr(pages.ptr);
            puts(con, "[UEFI] Allocated RAM Shadow Backbuffer: 0x");
            printHex64(con, boot_info.fb.shadow_base);
            puts(con, "\r\n");
        } else |_| {
            puts(con, "[WARN] Could not allocate RAM Shadow Backbuffer\r\n");
        }
    } else |err| {
        puts(con, "[WARN] UEFI GOP locateProtocol failed: ");
        printHex64(con, @intFromError(err));
        puts(con, "\r\n");
    }
    boot_info.sev_c_bit_mask = detectSevCBitMask();

    // 6. Discover AMD SEV-SNP Confidential Computing Secrets Page if present
    discoverSevSnpSecrets(st);

    // 6b. Preload ESP FAT32 partition sectors into RAM disk via UEFI BlockIo
    preloadStorageRamdisk(bs, con);

    const raw_stack_top = @intFromPtr(&handoff_stack) + handoff_stack.len - 8;
    const stack_top: u64 = if (raw_stack_top < 0x1_0000_0000) raw_stack_top else 0x003E_FFF8;

    // 7. Query UEFI Memory Map & Call ExitBootServices
    if (exitBootServicesAndPopulateMmap(bs) != .success) {
        return .aborted;
    }

    // 8. Disable interrupts, configure SEV-SNP C-bit + Unencrypted DMA + Framebuffer PDEs, and jump to kmain
    setupIdentityPageTablesWithUnencryptedDma(dma_phys, boot_info.fb);

    // Switch to a dedicated loader-owned stack, outside the DMA arena
    // RSP must be 16n + 8 on entry so push %rbp aligns RSP to 16 bytes
    // Pass boot_info in %rdi (SysV ABI) and %rcx (Microsoft x64 ABI)
    asm volatile (
        \\movq %[stack], %%rsp
        \\jmp *%[entry]
        :
        : [rdi] "{rdi}" (&boot_info),
          [rcx] "{rcx}" (&boot_info),
          [entry] "r" (kernel_entry),
          [stack] "r" (stack_top),
        : .{ .memory = true });
    while (true) {
        asm volatile ("hlt");
    }
}

// ============================================================================
// 3. ESP FILE SYSTEM READER (\EFI\TRISMEGISTUS\KERNEL.ELF)
// ============================================================================

fn preloadStorageRamdisk(bs: *uefi.tables.BootServices, con: *uefi.protocol.SimpleTextOutput) void {
    const loaded_img = (bs.openProtocol(
        uefi.protocol.LoadedImage,
        uefi.handle,
        .{ .by_handle_protocol = .{ .agent = uefi.handle } },
    ) catch return) orelse return;

    const dev_handle = loaded_img.device_handle orelse return;
    const bio = (bs.openProtocol(
        uefi.protocol.BlockIo,
        dev_handle,
        .{ .by_handle_protocol = .{ .agent = uefi.handle } },
    ) catch return) orelse return;

    // Allocate 16 MiB (4096 pages) below 4 GiB for ESP ramdisk
    const ramdisk_pages: usize = 4096;
    const max_low_ptr: [*]align(4096) uefi.Page = @ptrFromInt(0xFFFF_F000);
    const pages = bs.allocatePages(.{ .max_address = max_low_ptr }, .loader_data, ramdisk_pages) catch {
        puts(con, "[WARN] Could not allocate RAM for ESP Ramdisk\r\n");
        return;
    };

    const rd_slice: []u8 = @as([*]u8, @ptrCast(pages.ptr))[0 .. ramdisk_pages * 4096];
    const media_id = bio.media.media_id;
    const block_sz = bio.media.block_size;

    if (block_sz == 512) {
        bio.readBlocks(media_id, 0, rd_slice) catch {
            puts(con, "[WARN] BlockIo.readBlocks failed\r\n");
            return;
        };
        boot_info.ramdisk_phys = @intFromPtr(pages.ptr);
        boot_info.ramdisk_size = rd_slice.len;
        puts(con, "[UEFI] Preloaded 16 MiB ESP Storage Ramdisk: 0x");
        printHex64(con, boot_info.ramdisk_phys);
        puts(con, "\r\n");
    }
}

// ============================================================================
// 4. STRICT ELF64 PARSER & PHYSICAL SEGMENT LOADER
// ============================================================================

const PT_LOAD: u32 = 1;
const PF_X: u32 = 1;

const Elf64_Phdr = extern struct {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
};

const Elf64_Shdr = extern struct {
    sh_name: u32,
    sh_type: u32,
    sh_flags: u64,
    sh_addr: u64,
    sh_offset: u64,
    sh_size: u64,
    sh_link: u32,
    sh_info: u32,
    sh_addralign: u64,
    sh_entsize: u64,
};

const Elf64_Sym = extern struct {
    st_name: u32,
    st_info: u8,
    st_other: u8,
    st_shndx: u16,
    st_value: u64,
    st_size: u64,
};

fn findKmainEntry(elf_bytes: []const u8) ?u64 {
    if (elf_bytes.len < @sizeOf(elf.Elf64_Ehdr)) return null;
    const ehdr: *const elf.Elf64_Ehdr = @ptrCast(@alignCast(elf_bytes.ptr));
    if (!std.mem.eql(u8, ehdr.e_ident[0..4], "\x7fELF")) return null;
    if (ehdr.e_shoff == 0) return null;

    const shnum: usize = ehdr.e_shnum;
    const shentsize: usize = ehdr.e_shentsize;
    const shoff: usize = @intCast(ehdr.e_shoff);
    if (shentsize != @sizeOf(Elf64_Shdr) or shoff % @alignOf(Elf64_Shdr) != 0) return null;
    const shbytes = std.math.mul(usize, shnum, shentsize) catch return null;
    const shend = std.math.add(usize, shoff, shbytes) catch return null;
    if (shend > elf_bytes.len) return null;

    var maybe_symtab: ?*const Elf64_Shdr = null;
    var i: usize = 0;
    while (i < shnum) : (i += 1) {
        const sh_ptr: *const Elf64_Shdr = @ptrCast(@alignCast(&elf_bytes[shoff + i * shentsize]));
        if (sh_ptr.sh_type == 2) { // SHT_SYMTAB
            maybe_symtab = sh_ptr;
            break;
        }
    }

    const symtab = maybe_symtab orelse return null;
    if (symtab.sh_link >= shnum) return null;
    const strtab_sh: *const Elf64_Shdr = @ptrCast(@alignCast(&elf_bytes[shoff + @as(usize, symtab.sh_link) * shentsize]));

    const sym_off: usize = @intCast(symtab.sh_offset);
    const sym_sz: usize = @intCast(symtab.sh_size);
    const str_off: usize = @intCast(strtab_sh.sh_offset);
    const str_sz: usize = @intCast(strtab_sh.sh_size);

    const sym_end = std.math.add(usize, sym_off, sym_sz) catch return null;
    const str_end = std.math.add(usize, str_off, str_sz) catch return null;
    if (sym_end > elf_bytes.len or str_end > elf_bytes.len or sym_off % @alignOf(Elf64_Sym) != 0) return null;

    const sym_entsize = if (symtab.sh_entsize > 0) @as(usize, @intCast(symtab.sh_entsize)) else @sizeOf(Elf64_Sym);
    if (sym_entsize != @sizeOf(Elf64_Sym)) return null;
    const num_syms = sym_sz / sym_entsize;

    var s: usize = 0;
    while (s < num_syms) : (s += 1) {
        const sym: *const Elf64_Sym = @ptrCast(@alignCast(&elf_bytes[sym_off + s * sym_entsize]));
        const name_off: usize = @intCast(sym.st_name);
        if (name_off < str_sz) {
            const sym_slice = elf_bytes[str_off + name_off .. str_off + str_sz];
            const sym_name = std.mem.sliceTo(sym_slice, 0);
            if (std.mem.eql(u8, sym_name, "kmain")) {
                return sym.st_value;
            }
        }
    }
    return null;
}

fn printHex64(con: *uefi.protocol.SimpleTextOutput, val: u64) void {
    const hex = "0123456789abcdef";
    var shift: u6 = 60;
    while (true) {
        const nibble: usize = @intCast((val >> shift) & 0x0F);
        var buf: [2:0]u16 = .{ hex[nibble], 0 };
        _ = con.outputString(&buf) catch false;
        if (shift == 0) break;
        shift -= 4;
    }
}

fn loadElf64Segments(
    bs: *uefi.tables.BootServices,
    con: *uefi.protocol.SimpleTextOutput,
    elf_bytes: []const u8,
) !u64 {
    if (elf_bytes.len < @sizeOf(elf.Elf64_Ehdr)) return error.LoadError;
    const ehdr: *const elf.Elf64_Ehdr = @ptrCast(@alignCast(elf_bytes.ptr));

    // Verify ELF64 Magic, Class, Endianness, and x86_64 Machine Type
    if (!std.mem.eql(u8, ehdr.e_ident[0..4], "\x7fELF") or
        ehdr.e_ident[elf.EI_CLASS] != elf.ELFCLASS64 or
        ehdr.e_ident[elf.EI_DATA] != elf.ELFDATA2LSB or
        ehdr.e_machine != .X86_64)
    {
        return error.IncompatibleVersion;
    }

    if (ehdr.e_phentsize != @sizeOf(Elf64_Phdr) or ehdr.e_phoff % @alignOf(Elf64_Phdr) != 0) return error.LoadError;
    const phdr_size = std.math.mul(u64, ehdr.e_phnum, ehdr.e_phentsize) catch return error.LoadError;
    const phdr_end = std.math.add(u64, ehdr.e_phoff, phdr_size) catch return error.LoadError;

    if (phdr_end > elf_bytes.len) return error.LoadError;

    var i: usize = 0;
    while (i < ehdr.e_phnum) : (i += 1) {
        const ph_off: usize = @intCast(ehdr.e_phoff + (i * ehdr.e_phentsize));
        const phdr: *const Elf64_Phdr = @ptrCast(@alignCast(&elf_bytes[ph_off]));
        if (phdr.p_type != PT_LOAD or phdr.p_memsz == 0) continue;

        const paddr = if (phdr.p_paddr != 0) phdr.p_paddr else phdr.p_vaddr;

        // Skip the 2 MiB DMA section if it's marked PT_LOAD NOLOAD
        if (paddr == 0x0020_0000 and phdr.p_filesz == 0) continue;
        if (phdr.p_filesz > phdr.p_memsz) return error.LoadError;
        const file_end = std.math.add(u64, phdr.p_offset, phdr.p_filesz) catch return error.LoadError;
        if (file_end > elf_bytes.len) return error.LoadError;
        const memory_end = std.math.add(u64, paddr, phdr.p_memsz) catch return error.LoadError;
        if (memory_end > 0x1_0000_0000) return error.LoadError;

        const page_base = std.mem.alignBackward(u64, paddr, 4096);
        const page_end = std.mem.alignForward(u64, paddr + phdr.p_memsz, 4096);
        const num_pages: usize = @intCast((page_end - page_base) >> 12);

        const target_phys_ptr: [*]align(4096) uefi.Page = @ptrFromInt(page_base);
        const alloc_type: uefi.tables.MemoryType = if ((phdr.p_flags & PF_X) != 0) .loader_code else .loader_data;

        _ = bs.allocatePages(.{ .address = target_phys_ptr }, alloc_type, num_pages) catch {
            puts(con, "[FATAL] Physical address collision loading ELF PT_LOAD segment\r\n");
            return error.OutOfResources;
        };

        // Copy initialized file bytes (p_filesz)
        const dst_ptr: [*]u8 = @ptrFromInt(paddr);
        const filesz: usize = @intCast(phdr.p_filesz);
        const memsz: usize = @intCast(phdr.p_memsz);

        if (filesz > 0) {
            const src_off: usize = @intCast(phdr.p_offset);
            @memcpy(dst_ptr[0..filesz], elf_bytes[src_off .. src_off + filesz]);
        }

        // Zero trailing BSS / NOLOAD bytes (p_memsz - p_filesz)
        if (memsz > filesz) {
            @memset(dst_ptr[filesz..memsz], 0);
        }
    }

    puts(con, "[UEFI] Mapped all ELF64 PT_LOAD segments.\r\n");

    if (findKmainEntry(elf_bytes)) |kentry| {
        puts(con, "[UEFI] Resolved 'kmain' 64-bit entry point: 0x");
        printHex64(con, kentry);
        puts(con, "\r\n");
        return kentry;
    }
    return error.Missing64BitKernelEntry;
}

// ============================================================================
// 5. EXIT BOOT SERVICES & SEV-SNP DMA PAGE TABLE SWITCH
// ============================================================================

fn exitBootServicesAndPopulateMmap(bs: *uefi.tables.BootServices) uefi.Status {
    const mem_info = bs.getMemoryMapInfo() catch return .aborted;
    const needed_size = (mem_info.len + 16) * mem_info.descriptor_size;

    const mmap_buf = bs.allocatePool(.loader_data, needed_size) catch return .out_of_resources;

    // Retry loop as required by UEFI Specification
    var attempts: usize = 0;
    while (attempts < 3) : (attempts += 1) {
        const map_slice = bs.getMemoryMap(mmap_buf) catch return .aborted;

        if (bs.exitBootServices(uefi.handle, map_slice.info.key)) {
            // Translate UEFI descriptors into Trismegistus BootInfo
            var out_idx: u32 = 0;

            for (0..map_slice.info.len) |idx| {
                if (out_idx >= proto.MAX_MEMORY_REGIONS - 2) break;
                const desc = map_slice.getUnchecked(idx);
                static_regions[out_idx] = .{
                    .base_phys = desc.physical_start,
                    .length_bytes = desc.number_of_pages * 4096,
                    .region_type = switch (desc.type) {
                        .conventional_memory, .boot_services_code, .boot_services_data => .Usable,
                        .acpi_reclaim_memory => .AcpiReclaim,
                        .memory_mapped_io, .memory_mapped_io_port_space => .MmioVirtio,
                        else => .Reserved,
                    },
                    .flags = proto.RegionFlags.READ | proto.RegionFlags.WRITE,
                };
                out_idx += 1;
            }

            // Append explicit SharedDmaUnencrypted entry
            static_regions[out_idx] = .{
                .base_phys = boot_info.dma_pool_phys,
                .length_bytes = boot_info.dma_pool_size,
                .region_type = .DmaSharedPool,
                .flags = proto.RegionFlags.READ | proto.RegionFlags.WRITE | proto.RegionFlags.UNENCRYPTED_DMA,
            };
            out_idx += 1;

            boot_info.memory_map_ptr = &static_regions;
            boot_info.memory_map_entries = out_idx;
            boot_info.flags |= proto.BootFlags.UEFI_BOOT;
            return .success;
        } else |_| {}
    }

    return .aborted;
}

fn initGopFramebuffer(bs: *uefi.tables.BootServices, con: *uefi.protocol.SimpleTextOutput) !proto.FramebufferConfig {
    const gop_ptr = (bs.locateProtocol(uefi.protocol.GraphicsOutput, null) catch return error.GopNotFound) orelse return error.GopNotFound;

    var best_mode: u32 = gop_ptr.mode.mode;
    var i: u32 = 0;
    while (i < gop_ptr.mode.max_mode) : (i += 1) {
        if (gop_ptr.queryMode(i)) |info| {
            if ((info.horizontal_resolution == 1280 and info.vertical_resolution == 800) or
                (info.horizontal_resolution == 1024 and info.vertical_resolution == 768))
            {
                best_mode = i;
                break;
            }
        } else |_| {}
    }
    _ = gop_ptr.setMode(best_mode) catch {};

    const mode = gop_ptr.mode;
    puts(con, "[UEFI] GOP Framebuffer: ");
    printHex64(con, mode.info.horizontal_resolution);
    puts(con, "x");
    printHex64(con, mode.info.vertical_resolution);
    puts(con, " VRAM: 0x");
    printHex64(con, mode.frame_buffer_base);
    puts(con, "\r\n");

    return proto.FramebufferConfig{
        .base_addr = mode.frame_buffer_base,
        .buffer_size = mode.frame_buffer_size,
        .width = mode.info.horizontal_resolution,
        .height = mode.info.vertical_resolution,
        .pixels_per_scanline = mode.info.pixels_per_scan_line,
        .format = switch (mode.info.pixel_format) {
            .red_green_blue_reserved_8_bit_per_color => .rgbx8888,
            .blue_green_red_reserved_8_bit_per_color => .bgrx8888,
            else => .bitmask,
        },
    };
}

fn setupIdentityPageTablesWithUnencryptedDma(dma_phys: u64, fb: proto.FramebufferConfig) void {
    if (builtin.cpu.arch != .x86_64) return;
    asm volatile ("cli" ::: .{ .memory = true });

    const c_bit_mask = detectSevCBitMask();
    const dma_pde_index: usize = @intCast((dma_phys >> 21) & 0x1FF);

    // Ring-3 Userspace Access: Enable Bit 2 (U/S = 0x04) on 0..1 GiB mappings
    pml4[0] = @intFromPtr(&pdpt) | 0x07 | c_bit_mask;
    pdpt[0] = @intFromPtr(&pd_0) | 0x07 | c_bit_mask;
    pdpt[1] = @intFromPtr(&pd_1) | 0x03 | c_bit_mask;
    pdpt[2] = @intFromPtr(&pd_2) | 0x03 | c_bit_mask;
    pdpt[3] = @intFromPtr(&pd_3) | 0x03 | c_bit_mask;

    const fb_start = fb.base_addr;
    const fb_end = fb.base_addr + fb.buffer_size;

    const pds = [_]*[512]u64{ &pd_0, &pd_1, &pd_2, &pd_3 };
    for (0..4) |gb_idx| {
        const pd = pds[gb_idx];
        const gb_base: u64 = @as(u64, gb_idx) * 0x4000_0000;
        for (0..512) |i| {
            const phys: u64 = gb_base + (@as(u64, i) * 0x0020_0000);
            const is_dma = (gb_idx == 0 and i == dma_pde_index);
            const is_fb = (fb.base_addr != 0 and (phys + 0x0020_0000) > fb_start and phys < fb_end);

            if (is_dma or is_fb) {
                // Clear C-bit for hypervisor access. Use Write-Combining (PWT bit 3 = 0x08) on FB.
                const flags: u64 = if (is_fb) 0x8B else 0x83;
                const user_bit: u64 = if (gb_idx == 0) 0x04 else 0;
                pd[i] = phys | flags | user_bit;
            } else if (gb_idx == 3) {
                // MMIO window 3..4 GiB (PCD=1). For Local APIC (index 503, 0xFEE00000) enforce 0x93 (Strong Uncacheable)
                const flags: u64 = if (i == 503) 0x93 else 0x9B;
                pd[i] = phys | flags;
            } else {
                const user_bit: u64 = if (gb_idx == 0) 0x04 else 0;
                pd[i] = phys | 0x83 | user_bit | c_bit_mask;
            }
        }
    }

    // Map Above-4G GOP Framebuffer BAR (64-bit PCIe Resizable BAR / Above-4G Decoding)
    if (fb.base_addr >= 0x1_0000_0000 and fb.buffer_size > 0) {
        const fb_pml4_idx: usize = @intCast((fb.base_addr >> 39) & 0x1FF);
        const fb_pdpt_idx: usize = @intCast((fb.base_addr >> 30) & 0x1FF);
        const fb_gb_base: u64 = fb.base_addr & ~@as(u64, 0x3FFF_FFFF);
        for (0..512) |i| {
            const phys: u64 = fb_gb_base + (@as(u64, i) * 0x0020_0000);
            pd_fb_hi[i] = phys | 0x8B; // 2 MiB Write-Combining + User/Supervisor
        }
        if (fb_pml4_idx == 0) {
            pdpt[fb_pdpt_idx] = @intFromPtr(&pd_fb_hi) | 0x07 | c_bit_mask;
        } else {
            pml4[fb_pml4_idx] = @intFromPtr(&pdpt_fb_hi) | 0x07 | c_bit_mask;
            pdpt_fb_hi[fb_pdpt_idx] = @intFromPtr(&pd_fb_hi) | 0x07 | c_bit_mask;
        }
    }

    // Map UEFI Loader image itself if firmware placed BOOTX64.EFI above 4 GiB
    const ldr_addr: u64 = @intFromPtr(&boot_info);
    if (ldr_addr >= 0x1_0000_0000) {
        const ldr_pml4_idx: usize = @intCast((ldr_addr >> 39) & 0x1FF);
        const ldr_pdpt_idx: usize = @intCast((ldr_addr >> 30) & 0x1FF);
        const ldr_gb_base: u64 = ldr_addr & ~@as(u64, 0x3FFF_FFFF);
        for (0..512) |i| {
            const phys: u64 = ldr_gb_base + (@as(u64, i) * 0x0020_0000);
            pd_ldr_hi[i] = phys | 0x83 | c_bit_mask;
        }
        if (ldr_pml4_idx == 0) {
            if (pdpt[ldr_pdpt_idx] == 0) {
                pdpt[ldr_pdpt_idx] = @intFromPtr(&pd_ldr_hi) | 0x07 | c_bit_mask;
            }
        } else {
            if (pml4[ldr_pml4_idx] == 0) {
                pml4[ldr_pml4_idx] = @intFromPtr(&pdpt_ldr_hi) | 0x07 | c_bit_mask;
                pdpt_ldr_hi[ldr_pdpt_idx] = @intFromPtr(&pd_ldr_hi) | 0x07 | c_bit_mask;
            } else if (pml4[ldr_pml4_idx] == (@intFromPtr(&pdpt_fb_hi) | 0x07 | c_bit_mask)) {
                pdpt_fb_hi[ldr_pdpt_idx] = @intFromPtr(&pd_ldr_hi) | 0x07 | c_bit_mask;
            }
        }
    }

    // Configure IA32_PAT (MSR 0x277) so PA1 (PWT=1, PCD=0 -> 0x8B PDE) uses Write-Combining (0x01)
    programPatWriteCombining();

    // Load CR3 with our new PML4
    const pml4_phys = @intFromPtr(&pml4);
    asm volatile ("movq %[cr3], %%cr3"
        :
        : [cr3] "r" (pml4_phys),
        : .{ .memory = true });

    // If SEV-SNP is active (MSR 0xC0010131 bit 2), mark the 512 4KiB DMA frames Shared in RMP
    if (c_bit_mask != 0 and (rdmsr(0xC001_0131) & 0x4) != 0) {
        const GHCB_MSR: u32 = 0xC001_0130;
        for (0..512) |page_idx| {
            const pfn = (dma_phys + (page_idx * 4096)) >> 12;
            const req: u64 = (@as(u64, 2) << 52) | (pfn << 12) | 0x014;
            wrmsr(GHCB_MSR, req);
            asm volatile (".byte 0xF3, 0x0F, 0x01, 0xD9" ::: .{ .memory = true });
        }
    }
}

// ============================================================================
// 6. HARDWARE & CONSOLE HELPERS
// ============================================================================

fn detectSevCBitMask() u64 {
    if (builtin.cpu.arch != .x86_64) return 0;
    var eax: u32 = undefined;
    var ebx: u32 = undefined;
    var ecx: u32 = undefined;
    var edx: u32 = undefined;

    asm volatile ("cpuid"
        : [eax] "={eax}" (eax),
          [ebx] "={ebx}" (ebx),
          [ecx] "={ecx}" (ecx),
          [edx] "={edx}" (edx),
        : [leaf] "{eax}" (@as(u32, 0x8000_0000)),
    );

    if (eax < 0x8000_001F) return 0;

    asm volatile ("cpuid"
        : [eax] "={eax}" (eax),
          [ebx] "={ebx}" (ebx),
          [ecx] "={ecx}" (ecx),
          [edx] "={edx}" (edx),
        : [leaf] "{eax}" (@as(u32, 0x8000_001F)),
    );

    // Bit 1: AMD SEV feature supported
    if ((eax & 0x02) == 0) return 0;

    // Check MSR_AMD64_SEV (0xC0010131) bit 0 (SEV active)
    if ((rdmsr(0xC001_0131) & 1) == 0) return 0;

    const c_bit: u6 = @truncate(ebx & 0x3F);
    return @as(u64, 1) << c_bit;
}

fn discoverSevSnpSecrets(st: *uefi.tables.SystemTable) void {
    const tables = st.configuration_table[0..st.number_of_table_entries];
    for (tables) |entry| {
        if (entry.vendor_guid.eql(EFI_CC_BLOB_GUID)) {
            const blob: *const EfiCcBlobTable = @ptrCast(@alignCast(entry.vendor_table));
            boot_info.snp_secrets_phys = blob.secrets_paddr;
            boot_info.snp_cpuid_phys = blob.cpuid_paddr;
            return;
        }
    }
}

fn allocate2MibAligned(bs: *uefi.tables.BootServices) !u64 {
    // Allocate 4 MiB (1024 pages) below 1 GiB so a 2 MiB boundary is guaranteed inside PD_0
    const max_pd0_ptr: [*]align(4096) uefi.Page = @ptrFromInt(0x3FFF_F000);
    const pages = bs.allocatePages(.{ .max_address = max_pd0_ptr }, .loader_data, 1024) catch {
        return error.OutOfResources;
    };
    return std.mem.alignForward(u64, @intFromPtr(pages.ptr), 0x0020_0000);
}

fn puts(con: *uefi.protocol.SimpleTextOutput, comptime msg: []const u8) void {
    _ = con.outputString(L(msg)) catch false;
}

fn printSha384(con: *uefi.protocol.SimpleTextOutput, digest: *const [48]u8) void {
    const hex = "0123456789abcdef";
    puts(con, "[PYTHIA] Kernel SHA-384: ");
    var buf: [2:0]u16 = .{ 0, 0 };
    for (digest) |b| {
        buf[0] = hex[b >> 4];
        _ = con.outputString(&buf) catch false;
        buf[0] = hex[b & 0x0F];
        _ = con.outputString(&buf) catch false;
    }
    puts(con, "\r\n");
}

/// Program IA32_PAT MSR (0x277) so PAT index 1 (PWT=1, PCD=0 -> 0x8B PDE) uses Write-Combining (0x01).
fn programPatWriteCombining() void {
    if (builtin.cpu.arch != .x86_64) return;
    var eax: u32 = undefined;
    var ebx: u32 = undefined;
    var ecx: u32 = undefined;
    var edx: u32 = undefined;
    asm volatile ("cpuid"
        : [eax] "={eax}" (eax),
          [ebx] "={ebx}" (ebx),
          [ecx] "={ecx}" (ecx),
          [edx] "={edx}" (edx),
        : [leaf] "{eax}" (@as(u32, 1)),
    );
    if ((edx & (@as(u32, 1) << 16)) == 0) return;
    wrmsr(0x0000_0277, 0x00070406_00070106);
}

inline fn rdmsr(msr: u32) u64 {
    var low: u32 = undefined;
    var high: u32 = undefined;
    asm volatile ("rdmsr"
        : [low] "={eax}" (low),
          [high] "={edx}" (high),
        : [msr] "{ecx}" (msr),
    );
    return (@as(u64, high) << 32) | low;
}

inline fn wrmsr(msr: u32, val: u64) void {
    asm volatile ("wrmsr"
        :
        : [msr] "{ecx}" (msr),
          [low] "{eax}" (@as(u32, @truncate(val))),
          [high] "{edx}" (@as(u32, @truncate(val >> 32))),
    );
}
