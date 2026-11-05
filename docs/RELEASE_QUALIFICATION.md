# Release qualification and support diagnostics

Well's packaged executable provides non-GUI commands that run before window,
GPU, PTY, configuration, or AI initialization:

```bash
dist/Well.app/Contents/MacOS/Well --version
dist/Well.app/Contents/MacOS/Well --diagnose
```

`--diagnose` prints a versioned JSON health report suitable for an issue or
support ticket. It reports application and target versions, whether the binary
is inside a macOS app bundle, configuration validity, shell presence and
availability, and whether provider environment variables are present.

The report deliberately excludes credential values, environment values,
terminal output, command history, scrollback, file contents, hostname, current
working directory, shell path, executable path, and OS Keychain contents. Known
application paths below the home directory are written with a `$HOME` prefix.
Review any report before sharing it.

## Packaged-app acceptance test

After packaging, run:

```bash
./scripts/verify-release-candidate.sh dist/Well.app
```

The acceptance test first performs the full bundle verification, then runs the
packaged binary under an isolated empty home directory. It validates `--version`,
parses the diagnostic JSON, checks its privacy assertions and clean-config
status, and confirms that diagnostics created no config or runtime state.

CI runs this same test against the app bundle before uploading release
artifacts. Interactive terminal compatibility remains covered separately by
`scripts/smoke-interactive-programs.sh` because those programs depend on local
host tools and a real PTY.

For a local release-candidate pass, pair the packaged acceptance test with the
normal workspace gates:

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
./scripts/preflight-macos-release.sh
./scripts/smoke-shell-startup.sh
./scripts/smoke-interactive-programs.sh
```

For the final notarized release candidate, run the stricter signing/notary
preflight with the same environment used by `package-app.sh --notarize`:

```bash
WELL_SIGN_IDENTITY="Developer ID Application: Your Name or Company (TEAMID)" \
WELL_NOTARY_PROFILE=well-notary \
WELL_BUILD_NUMBER=42 \
./scripts/preflight-macos-release.sh --notarize
```

## GUI launch smoke test

After diagnostics pass, run the packaged GUI long enough to catch immediate
window, GPU, PTY, or startup-config crashes:

```bash
./scripts/smoke-gui-launch.sh dist/Well.app
```

The smoke test launches the packaged executable under an isolated empty home
directory, waits 8 seconds by default, and treats a still-running process as a
successful startup. Set `WELL_GUI_SMOKE_SECONDS` to adjust the wait time.
