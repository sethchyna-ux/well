# Well

Well is an experimental Rust terminal workspace. The current production target is a native macOS terminal app built around a real PTY session, `vt100` screen parsing, and a `wgpu` renderer.

The repository also contains exploratory subsystems for AI command assistance, mobile/Flutter wrappers, remote/headless execution, Firebase/Data Connect sync, WebAssembly plugins, SSH, and metrics. Those areas are not treated as production-ready unless explicitly called out in this README.

## Current production spine

The v1 path is intentionally narrow:

- desktop terminal host in `src/`
- canonical interactive shell path through `crates/well-shell::pty::PtySession`
- one interactive desktop terminal session with configurable startup scrollback
- terminal cell rendering through `crates/well-render`
- configuration/control-panel work through `crates/well-config`
- IPC and server primitives through `crates/well-ipc` and `crates/well-server`

Mobile, cloud/Kubernetes, image generation, and remote execution are experimental until the core desktop terminal is stable.
Tabs/panes, Kitty image rendering, and sandbox replay are deferred rather than advertised as active desktop features.

## Build

Prerequisites:

- macOS 12+ for the primary desktop target
- Rust stable
- Zig available on `PATH` if you need the optional vendored Ghostty archive build path

Common commands:

```bash
cargo check --workspace
cargo test --workspace
cargo run --bin well
```

Package a local macOS app bundle:

```bash
./package-app.sh
```

The package script performs a locked build, signs and verifies `dist/Well.app`, and does not install into `/Applications` unless you explicitly pass `--install`. Use `--archive` to create the versioned macOS ZIP and checksum.

```bash
./package-app.sh --archive
./package-app.sh --install
```

Generate the Homebrew Cask from the same versioned ZIP and checksum:

```bash
./package-app.sh --archive --cask
```

For public distribution, generate the Cask from a Developer ID-signed and
notarized release. See [Homebrew distribution](docs/HOMEBREW.md) for the tap
promotion and install path.

For bundle metadata, Developer ID signing, notarization, CI artifacts, and the release checklist, see the canonical [release signing guide](docs/RELEASE_SIGNING.md).

## Verification status

The current stabilization baseline is:

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
./scripts/check-repo-hygiene.sh
```

The workspace uses a local `[patch.crates-io]` for `block v0.1.6` under `vendor/block-0.1.6` to remove a Rust future-incompatibility warning inherited through the `wgpu`/`metal` dependency stack.

## Repository hygiene

Generated/local artifacts should not be committed:

- `node_modules/`
- `.DS_Store`
- logs
- generated Android CLI/FFI binaries
- local generated media

Use package managers and build scripts to regenerate those artifacts locally.

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) describes current subsystem boundaries.
- [ROADMAP.md](ROADMAP.md) tracks the production-readiness plan.
- [docs/RUNTIME_LOCATIONS.md](docs/RUNTIME_LOCATIONS.md) lists config, generated image, log, and package locations.
- [docs/RELEASE_SIGNING.md](docs/RELEASE_SIGNING.md) documents Developer ID signing and notarization.
- [docs/RELEASE_QUALIFICATION.md](docs/RELEASE_QUALIFICATION.md) documents clean-state packaged-app diagnostics and acceptance testing.
- [docs/AI_SAFETY.md](docs/AI_SAFETY.md) documents Pythia provider data flow, credential storage, and command safety gates.
- [MYTHOLOGY.md](MYTHOLOGY.md) contains broader naming and conceptual background.

## License

MIT. See [LICENSE](LICENSE).
