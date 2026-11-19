# Implementation audit — 2026-11-14

This is a source and runtime audit, not a certification. Status changes require evidence.
The GPUI desktop is the current product; the root wgpu desktop, Flutter app and
cloud console are separate legacy/experimental surfaces. Test fixtures and snippet
parameter placeholders are not production simulations.

| Priority | Surface / evidence | Finding | Acceptance / status |
|---|---|---|---|
| P0 | GPUI `ssh_console::DiscoveredSshKey` | Fabricated SHA256 fingerprints and a fabricated fallback identity | Genuine SHA256 over decoded key; empty key directory stays empty. Implemented; regression verification recorded below. |
| P0 | GPUI `main::main`, IPC `rpc::ensure_private_directory` | `/tmp` endpoint violates private-parent requirement; bind errors swallowed | Shared per-user private endpoint, observable startup failure. Implemented; regression verification recorded below. |
| P0 | `well-ssh::client::ArgusClient` | Accepts all host keys; connect helper neither authenticates nor opens subsystem | Implemented: native connector requires known-host match, authentication and accepted subsystem request. Desktop keeps interactive OpenSSH. |
| P1 | GPUI ServerHosting start handler | Only changes `server_running` boolean | Implemented: dedicated listener, authorized keys, optional password verifier, PTY bridge, exit status and stop. Real OpenSSH exec/rejection/shutdown test passed. |
| P1 | GPUI PXE HTTP server | Returns script for every request; no payload serving | Removed the HTTP-only script responder. The real companion serves named artifacts, 404s and status; complete HTTP/TFTP transfers and firmware boots passed. |
| P1 | GPUI CustomBootloader panel | Descriptive cards and synthetic protocol calls without firmware build target | Real AgentOS loader, ESP, QEMU and PXE boot. Implemented; regression verification recorded below. |
| P1 | GPUI IPC command dispatch | Ignores surface IDs; discards PTY write errors; reports SSH connected on process creation | Implemented: stable surface IDs, actual screen contents, missing-surface errors and PTY write results. SSH reports process_started rather than connected. |
| P1 | GPUI settings save handlers | Discard save errors; disconnected runtime preferences | Persistent settings and truthful failures. Implemented; regression verification recorded below. |
| P1 | GPUI defaults | Hard-coded hosts/tunnels and demo key | Empty real discovery, no seeded operational resources. Implemented; regression verification recorded below. |
| P1 | GPUI terminal color conversion | Maps all indexed colors modulo 16 | Full xterm cube and grayscale. Implemented; regression verification recorded below. |
| P1 | `well-wasm` | Trap isolation but no fuel/instance-memory budget | Implemented: per-call fuel, memory/table/instance limits, including start. Infinite-loop and oversized-memory regressions passed. GPUI plugin activation remains experimental and is not advertised as active. |
| P2 | GPUI SMB `is_mounted` | Directory existence shown as mounted | Verify actual SMB/CIFS mount table. Implemented; regression verification recorded below. |
| P2 | GPUI modules and legacy frontend | Binary redeclares library modules; overlapping frontend behavior | Binary imports library; consolidate after parity verification. Implemented; regression verification recorded below. |
| P2 | README/architecture/roadmap | Disagree about current frontend and feature readiness | Updated current frontend, ownership, deployment and qualification documentation. |
| P2 | IPC integration tests | Two concurrent framing failures; isolated serial pass | Implemented: private unique socket fixtures, bind-before-spawn readiness; workspace IPC regression suite passes. |

## Initial verification

- `cargo check --workspace --locked --offline`: passed.
- Selected config/editor/GPUI tests: passed (29 GPUI tests).
- Selected IPC run: two framing failures; serial framing rerun: five passed.
- `cargo fmt --all -- --check`: failed before changes.
- AgentOS `microvm` Zig ReleaseSafe build: passed with Zig 0.17.1.
- Older custom-boot Zig build: failed; different boot ABI, not the selected source.
- No merge markers found in initial source scan.
- Six staged files existed before implementation; preserve their contents and index.

## Delivery boundaries

Canonical loader belongs in Well; AgentOS owns its kernel and shared boot protocol.
First verified architecture is x86_64. Network modes: ProxyDHCP by default,
standalone DHCP on an explicitly chosen isolated interface. Privileged companion
runs through explicit sudo in a terminal pane. Physical hardware, ARM64, Secure Boot,
confidential-computing attestation, USB writing and public notarized distribution
require separate evidence. Hash checking is not hardware attestation.

## Additional second-pass findings and fixes

- Interface and ARP helpers invented LAN devices after discovery failed. Removed those fallback records and changed unavailable address detection to unspecified, which the netboot companion rejects.
- Tunnel Start/Stop only persisted a boolean. It now owns an actual OpenSSH PTY, reports process liveness and terminates its owned child. A running process is not presented as proof of a working remote listener.
- Incoming authorized-key rows listed local identity keys. They now show the actual authorized_keys file; Add validates and appends a clipboard public key and requires restarting the host.
- Local image insertion wrote Kitty output escapes to shell stdin. It now paints a bounded real overlay through the same rendering helper used by received image events.
- Two graphics interceptors had diverged; the shell copy lost dimensions and chunk inheritance. One canonical interceptor now lives in well-shell and is re-exported by well-render. Fragmented ESC, continuation metadata, deletion controls and bounded incomplete transfers have regressions.
- Generated images had no GPUI history lifecycle. Credential-free prompt/provider metadata now supports selection, regeneration and cache-scoped deletion; symlink/escape deletion regressions pass. Live provider generation still needs explicit credentials and separate qualification.
- GPUI operator persistence was unversioned and used a fixed temporary filename. Storage now lives in well-config with legacy-version reads, atomic replacement, writer locking and stale/future/malformed-file protection. Bindings are merged by action ID when the catalog evolves.
- Saved shortcuts did not drive actual bindings. Runtime registration now uses the catalog and saved values, with fallible parsing instead of startup panics.
- The native build CI invoked packaging flags the harness ignored. The harness now implements those flags, verifies the separate companion workspace, and packages both binaries plus loader sources. Linux output is a tar archive, reflected in CI artifact paths.
- The PXE project's latest pinned candidate did not compile: duplicate HTTP handlers and missing client-state variables were introduced during a merge. Well pins the earlier compiling revision b1e7bfb4658f5c6dce8ad8ab3893c45b141c0845 rather than silently modifying a dependency checkout.
- The local PXE file named ipxe.efi identifies itself as SNPONLY. A full upstream iPXE bootstrap was required for the emulated NIC test and its generated ESP is included in artifact checksums. No downloaded firmware binary is committed.

## Implemented versus remaining boundaries

| Subsystem | Current implementation and limits |
|---|---|
| GPUI | Actual PTYs, tabs/panes, editing, themes, SSH/NetOps and AgentOS panel. Visual qualification and legacy parity are distinct from headless tests. |
| Shell/core | Real shell execution, query handling, lifecycle, bounded I/O and warning heuristics. Warning detection is not an authorization boundary or sandbox. |
| Hermes | Authenticated local transport and runtime methods; unsupported methods remain explicit. Remote GPU orchestration is not implemented. |
| Mneme | Real rope/Tree-sitter/snippet operations and execution-time safety preferences. External LSP and remote editing are not claimed complete. |
| Astraea | Git/state snapshots; replacing the user's shell prompt remains deferred. |
| Config | Versioned shared preferences, profiles, and versioned GPUI operator storage. The GPUI operator layer is authoritative for its fields; shared typography is synchronized on save. |
| Rendering | GPUI plus shared parser/fonts; legacy GPU renderer retained pending parity. Kitty ACK negotiation, full placement/scroll semantics and hardware GPU coverage need deeper qualification. |
| Pythia | Opt-in provider calls, real cached artifacts/history, scoped deletion, direct image overlays. Provider-backed cancellation and live-provider coverage need further work. |
| SSH | OpenSSH outbound sessions/tunnels and authenticated incoming PTYs. System secret storage is real; hardware-backed signing is not implemented by that storage wrapper. |
| Wasm | Actual bounded wasmi execution. Automatic GPUI plugin activation is not enabled. |
| History/server/CLI | Real local execution/replay/streaming primitives; these do not imply a remote workspace platform. |
| AgentOS | Canonical x86_64 EFI loader, kernel integrity, ESP export, QEMU boots, and real HTTP/TFTP deployment. Physical ProxyDHCP/router behavior, ARM64, Secure Boot signing and hardware attestation remain unqualified. |
| FFI/mobile/web/cloud | Separate experimental or legacy surfaces, outside this production desktop delivery. |

The old frontend is retained deliberately: its independent behavior and regression
coverage must be migrated before deleting it. Formatting changes outside the active
frontend establish a passing workspace rustfmt baseline; they are not functional rewrites.

## Final verification and delivered artifacts

Verified on this macOS arm64 host on 2026-11-14:

- Workspace: 233 tests passed, zero failed, two ignored. Tests that require provider credentials do not establish live-provider coverage without those credentials.
- Workspace formatting, locked check and strict all-target Clippy passed. The isolated well-boot workspace passed formatting, strict Clippy and its FAT image regression.
- Shell startup and interactive-program smoke scripts passed.
- Final packaged app passed signature verification, clean-state diagnostics and eight-second clean-home GUI startup. The app is ad-hoc signed; it is not a notarized public release.
- The packaged companion built AgentOS while launched outside the repository. Its bundled loader source hash matches the canonical source and the export manifest.
- Disk firmware boot and boots through the real companion HTTP and TFTP listeners reached AgentOS, validated the handoff and returned the expected SHA256 of `WellBootVerified`. The service regression also checks HTTP 404, request metrics and a complete TFTP payload checksum.
- Settings and command-palette screenshots were inspected in the running app. Complete interactive AgentOS-panel, shortcut-remapping and responsive-layout qualification remains pending; automated keyboard navigation did not reliably establish those flows.

Deliverables are `dist/Well.app`, `dist/Well-0.1.0-macOS-arm64.zip` and its
checksum, plus `dist/AgentOS-x86_64/BOOTX64.EFI`, `agentos-esp.img`,
`netboot-bootstrap.img`, kernel ELF, `manifest.json` and `SHA256SUMS`.
Firmware and service transcripts are retained beside the AgentOS artifacts.
The AgentOS source checkout was dirty before this work; the manifest records
that fact rather than implying a clean upstream kernel build.

The companion HTTP counter is incremented by the actual request middleware;
it is covered by the live service check. The original staged index remains
six files (236 insertions, 33 deletions). AgentOS integration also changes
`/Volumes/remove/agent-os/microvm/build.zig`; the equivalent reviewable patch
is included in `boot/agentos/agentos-integration.patch`.

Remaining refinement priorities: complete GUI interaction qualification, resolve
fixed shortcut alias collisions and catalog hints after remapping, qualify full
Kitty placement/scroll/ACK behavior, provider cancellation, legacy parity, and
physical DHCP/ProxyDHCP behavior. These remain explicit work, not completed claims.

### UI and shortcut follow-up

Runtime bindings now use one shortcut parser for registration and conflict
checking. The command palette search, footer hints, header labels, and modal
buttons read the saved binding for each action instead of displaying stale
defaults. Empty bindings remain disabled, equivalent modifier spellings collide,
and fixed global aliases were removed so remapping can take effect. Settings
changes re-register bindings only when the binding catalog changes.

### Cursor flashing follow-up

The supplied recording exposed a modal focus oscillation in WorkspaceView render:
an already-focused palette or SSH modal fell through to terminal focus on the next
redraw. Focus owner selection is now independent of the already-focused check,
and terminal cursor ticks skip redraw notifications behind a workspace modal.
GPUI tests (26), strict Clippy, rebuilt packaging and release diagnostics pass.
This also addresses the likely cause of the earlier unreliable modal navigation.
Visual runtime confirmation remains pending because the automated capture returned
a reduced WindowServer thumbnail during this follow-up.
