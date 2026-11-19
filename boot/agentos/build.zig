const std = @import("std");
pub fn build(b: *std.Build) void {
    const kernel_path = b.option([]const u8, "kernel", "AgentOS x86_64 kernel ELF (absolute path)") orelse @panic("-Dkernel=<absolute ELF path> is required");
    const protocol_path = b.option([]const u8, "protocol", "Canonical AgentOS boot protocol (absolute path)") orelse @panic("-Dprotocol=<absolute protocol.zig path> is required");
    const target = b.resolveTargetQuery(.{ .cpu_arch = .x86_64, .os_tag = .uefi, .abi = .msvc });
    const options = b.addOptions();
    const elf_bytes = std.Io.Dir.cwd().readFileAlloc(b.graph.io, kernel_path, b.allocator, .limited(64 * 1024 * 1024)) catch @panic("Cannot read kernel ELF");
    var digest: [48]u8 = undefined;
    std.crypto.hash.sha2.Sha384.hash(elf_bytes, &digest, .{});
    options.addOption([48]u8, "EXPECTED_KERNEL_SHA384", digest);
    const exe = b.addExecutable(.{
        .name = "BOOTX64",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = b.standardOptimizeOption(.{ .preferred_optimize_mode = .ReleaseSafe }),
        }),
    });
    exe.root_module.addOptions("kernel_hash", options);
    exe.root_module.addAnonymousImport("kernel_payload", .{ .root_source_file = .{ .cwd_relative = kernel_path } });
    exe.root_module.addImport("protocol", b.createModule(.{ .root_source_file = .{ .cwd_relative = protocol_path }, .target = target }));
    b.installArtifact(exe);
}
