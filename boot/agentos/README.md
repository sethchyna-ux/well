# AgentOS loader owned by Well

Migrated from AgentOS microvm/boot/uefi_main.zig, revision
95fc9fe623eb159a1f4cc6df3acd3ca268caea4f. AgentOS continues to own the kernel
and microvm/boot/protocol.zig contract. Its build now consumes this loader.

Requires Zig 0.17.1. x86_64 only. The kernel is embedded and its SHA-384 digest
is checked before ELF loading. This verifies payload integrity, not hardware
attestation or Secure Boot trust.

Build/export through `well-boot build --source /path/to/agent-os --output /path/to/artifacts`.
Boot through `well-boot run --output /path/to/artifacts`; add `--network` to test
firmware network loading with QEMU's isolated DHCP/TFTP backend. This test does
not validate the separate Well netboot companion's DHCP/TFTP implementation.
The exported image is a deterministic FAT16 removable ESP volume, not a GPT disk.

The `serve` command uses the pinned existing PXE library for DHCP/TFTP and an
artifact-only HTTP server. No dashboard, download controller, or simulation route
is exposed. Use `--bind-ip` to select a real interface. Standalone DHCP requires
an explicit lease range and an isolated network. Starting a service is an explicit
operator action; building and testing never starts a DHCP server on the LAN.

Repeatable real runtime checks:

```bash
ruby scripts/verify-agentos-boot.rb target/debug/well-boot target/agentos-artifacts
ruby scripts/verify-agentos-boot.rb target/debug/well-boot target/agentos-artifacts network
ruby scripts/verify-agentos-services.rb target/debug/well-boot target/agentos-artifacts
```

The service check binds only nonstandard test ports, transfers the complete EFI file,
and boots QEMU through the actual companion HTTP and TFTP listeners. QEMU supplies
the guest's isolated initial DHCP lease. This does not claim physical ProxyDHCP/router
compatibility. The scripts assert a real AgentOS SHA-256 applet response and retain
serial logs. Full iPXE bootstrap binaries are available from the upstream
[download service](https://ipxe.org/download); keep provenance and checksums with exports.

For an AgentOS checkout preceding this migration, review and apply
`agentos-integration.patch` from this directory in the AgentOS root. It adds the
kernel-only target and directs the full AgentOS build to Well's canonical loader.
The default assumes sibling `agent-os` and `well` checkouts; the full AgentOS build
accepts `-Dwell-loader=/absolute/path/to/well/boot/agentos/src/main.zig` for another
layout. The companion supplies the canonical protocol path explicitly.
