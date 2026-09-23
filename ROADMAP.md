# Well production-readiness roadmap

## Phase 1: repo and build hygiene

Status: baseline complete; automated regression checks are active.

- Keep generated/local artifacts out of Git and enforce that boundary in CI.
- Keep `Cargo.lock` tracked for reproducible application builds.
- Maintain a clean `cargo check --workspace`.
- Enforce workspace check, full tests, rustfmt, and warning-free Clippy in CI.
- Keep README focused on current behavior; use this file for future plans.

Exit criteria:

- Fresh clone builds from source.
- No `node_modules`, logs, OS metadata, generated media, or native build products are tracked.
- Core verification commands pass.

## Phase 2: desktop terminal core

Status: core complete for one interactive session; PTY lifecycle, terminal
queries, configured startup scrollback, and common interactive program
compatibility have repeatable coverage.

- Make `PtySession` the canonical interactive runtime.
- Maintain deterministic PTY lifecycle coverage for spawn, resize, write, EOF, and child exit.
- Add terminal query responder tests for DA1, DA2, DSR, and bracketed paste.
- Verify common interactive programs: `vim`, `less`, `ssh`, `tmux`, `top`.

Exit criteria:

- Well can be dogfooded as a daily macOS terminal for ordinary shell work.

## Phase 3: rendering correctness

Status: baseline complete; rendering fixtures, Unicode glyph resolution, Kitty
image overlays, and GPU failure policies are covered.

- Add fixtures for colors, cursor styles, wrapping, wide Unicode, emoji fallback, combining characters, and alternate screen.
- Keep visual effects optional.
- Add graceful handling for GPU/device/surface failures.
- Keep Kitty graphics protocol image rendering bounded by decoded payload size
  and active-image limits.

Exit criteria:

- Common terminal programs render correctly and resizing does not crash.

## Phase 4: config stability

Status: baseline complete; JSON is canonical, schema migrations and safe
fallbacks are active, and Hyprlang round trips every non-secret setting.

- Define a versioned config schema.
- Add migration and invalid-config fallback.
- Decide whether JSON or Hyprlang is canonical.
- Add round-trip tests for Hyprlang import/export.
- Keep profile create/load/delete flows backed by validated local JSON files.

Exit criteria:

- Users cannot brick startup with a malformed config file.

## Phase 5: packaging and release

Status: baseline complete; locked staged builds, bundle verification, versioned
release archives, Developer ID signing, and notarization hooks are active.

- Produce `dist/Well.app` reproducibly.
- Keep `/Applications` installation opt-in.
- Document log/config/generated image/package locations.
- Add signing/notarization path.

Exit criteria:

- A clean machine can build and launch the app bundle.
- Runtime state locations are documented for support and cleanup.
- Developer ID signing and notarization steps are documented and the package script supports a signing identity.

## Phase 6: safety and AI

Status: baseline complete; AI defaults to offline operation, direct execution
revalidates generated commands, provider data flow is disclosed, and credentials
use environment variables or OS-backed secure storage. Provider-backed image
generation now records local artifact history and exposes explicit failure
states, but remains opt-in until terminal dogfooding is complete.

- Keep AI features opt-in.
- Gate destructive commands behind typed confirmation, including wrapped and compound shell commands.
- Document what terminal/context data leaves the machine for each provider.
- Keep credentials out of persisted plaintext config; add OS-backed keychain storage before stable release.
- Keep generated image artifacts manageable through local history, reveal/open/copy/regenerate/delete actions, and cache-local deletion guards.

Exit criteria:

- AI helpers are safe enough for beta users.
- Provider data flow and destructive-command behavior are documented for users.

## Phase 7: release qualification and operability

Status: baseline complete; the packaged executable exposes privacy-redacted,
non-GUI diagnostics and CI validates them from an isolated clean state.

- Provide version and health diagnostics without initializing the GUI, GPU, PTY, or AI.
- Keep support output free of credentials, terminal contents, history, and environment values.
- Exercise the packaged executable under an empty home directory before publishing artifacts.

Exit criteria:

- Support can obtain a versioned diagnostic report without launching Well.
- The same clean-state packaged-app acceptance test runs locally and in CI.
- Diagnostics do not create user state or expose provider secrets.

## Deferred until core is stable

- Mobile app productionization.
- Kubernetes/cloud runner.
- Remote terminal product.
- WebAssembly plugin ecosystem.
- Multi-session tabs/panes.
- Sandboxed replay/forking from scrollback history.
- Configurable renderer font families and CRT post-processing controls.
- Desktop PTY integration for Astraea prompt replacement and Mneme inline composition.
