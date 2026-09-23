# Agent context for Well

This file is the quick handoff for agents working in this repository. Read it
first, then use `README.md`, `ROADMAP.md`, and the relevant subsystem docs for
deeper detail.

## Project focus

Well is an experimental Rust terminal workspace. The current production target
is narrow: a native macOS terminal app built around a real PTY session,
`vt100` parsing, and a `wgpu` renderer.

Treat these as the production spine:

- `src/` for the desktop app entrypoint and GUI host.
- `crates/well-shell/` for PTY lifecycle, terminal query responses, and shell
  compatibility.
- `crates/well-render/` for terminal rendering and Kitty image display support.
- `crates/well-config/` for versioned config, profiles, and local state.
- `crates/well-editor/` for Mneme inline editor primitives.
- `crates/well-ipc/` and `crates/well-server/` for Hermes IPC/server
  primitives.
- `crates/well-llm/` plus GUI integration for opt-in Pythia AI/image features.

Mobile, cloud/Kubernetes, web console, remote execution, and plugin ecosystem
work exists in the repo, but it is not the current production path unless the
user explicitly asks for it.

## Current roadmap state

Baseline production-readiness phases in `ROADMAP.md` are now mostly complete:

- repo/build hygiene
- desktop PTY terminal core
- rendering correctness, including bounded Kitty image handling
- config stability and profile persistence
- macOS packaging, archive, Cask generation, Developer ID/notarization hooks
- AI safety basics and local generated-image artifact management
- packaged-app diagnostics and clean-state acceptance tests

The remaining production-risk areas are maturity and depth, not first-pass
existence:

- Mneme inline editor: the GUI composer now syncs through `MnemeEditor` and
  has tested execution-payload behavior; continue toward richer editing flows
  only when requested.
- Astraea prompt/state-vector: Git state now covers branch, detached HEAD,
  packed refs, linked worktrees, in-progress operations, and explicit dirty
  confidence without spawning `git`; prompt replacement/state propagation is
  still deferred unless requested.
- Hermes IPC/GPU engine maturity: benchmark, harden, and avoid overclaiming
  unimplemented remote/control surfaces.
- Profiles: continue making profile import/export/switching production-grade.
- Image generation: provider-backed generation works as an opt-in flow with
  local artifact history; keep improving failure states, asset lifecycle, and
  terminal insertion/rendering polish.
- Release trust: notarized distribution now has a preflight, but real public
  release still requires a Developer ID cert and valid Apple notary profile.

## Recent important changes

The latest release-trust work added:

- `scripts/preflight-macos-release.sh`
  - local macOS packaging prerequisite checks
  - stricter `--notarize` checks for Developer ID identity and
    `WELL_NOTARY_PROFILE`
  - optional `--skip-notary-service-check`
- `package-app.sh --notarize` now runs that preflight before compiling.
- Release/Homebrew docs reference the preflight:
  - `docs/RELEASE_SIGNING.md`
  - `docs/HOMEBREW.md`
  - `docs/RELEASE_QUALIFICATION.md`

The latest known package verification pass rebuilt `dist/Well.app`, generated
the arm64 ZIP/checksum/Cask, and passed clean-state diagnostics plus GUI smoke.

## Normal verification commands

Use a level of verification proportional to the change. For Rust/source
changes, prefer:

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

For terminal compatibility changes:

```bash
./scripts/smoke-shell-startup.sh
./scripts/smoke-interactive-programs.sh
```

For packaged app/release changes:

```bash
./scripts/preflight-macos-release.sh
./package-app.sh --archive --cask
./scripts/verify-release-candidate.sh dist/Well.app
./scripts/smoke-gui-launch.sh dist/Well.app
```

For final notarized release preparation on a machine with Apple credentials:

```bash
WELL_SIGN_IDENTITY="Developer ID Application: Your Name or Company (TEAMID)" \
WELL_NOTARY_PROFILE=well-notary \
WELL_BUILD_NUMBER=42 \
./scripts/preflight-macos-release.sh --notarize
```

Then build with:

```bash
WELL_SIGN_IDENTITY="Developer ID Application: Your Name or Company (TEAMID)" \
WELL_NOTARY_PROFILE=well-notary \
WELL_BUILD_NUMBER=42 \
./package-app.sh --notarize --cask
```

## Working rules for agents

- Preserve user changes. Do not reset, checkout, or delete work unless the user
  explicitly requests it.
- Use `rg`/`rg --files` for search.
- Prefer small, direct changes over broad rewrites.
- Do not claim experimental subsystems are production-ready unless the tests and
  docs prove it.
- Keep AI/image features opt-in and avoid persisting secrets in plaintext.
- Generated/local artifacts should generally stay out of source commits:
  `node_modules/`, `.DS_Store`, logs, generated media, native build products,
  and local runtime state.
- The package script must not install into `/Applications` unless `--install` is
  explicitly supplied.
- Homebrew distribution is a Cask for the app, not a formula-first CLI package.

## Useful docs

- `README.md` — current production spine and common commands.
- `ROADMAP.md` — phase status and deferred scope.
- `ARCHITECTURE.md` — subsystem boundaries.
- `docs/RUNTIME_LOCATIONS.md` — config/log/generated image/package paths.
- `docs/AI_SAFETY.md` — provider data flow and command safety.
- `docs/RELEASE_SIGNING.md` — signing, notarization, and release checklist.
- `docs/RELEASE_QUALIFICATION.md` — clean-state packaged-app diagnostics.
- `docs/HOMEBREW.md` — Cask generation and tap promotion.
- `.agents/agents/` and `.github/agents/` — specialized agent profiles.

## Suggested next task order

If the user says “next” without more detail, continue in this order unless the
current repo state clearly points elsewhere:

1. Hermes IPC/GPU maturity and honest capability boundaries.
2. Profile import/export/switching polish.
3. Image generation polish: lifecycle, insertion, rendering, and error states.
4. Mneme richer editing flows, if the user wants to go deeper.
5. Astraea prompt replacement/state propagation, if requested.
6. Release readiness: notarized public build, Cask promotion, and support docs.
