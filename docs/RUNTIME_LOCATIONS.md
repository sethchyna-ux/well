# Runtime locations

Well currently uses a small set of local filesystem locations. These are important for packaging, support, and cleanup.

## macOS app bundle

Local package builds write:

```text
dist/Well.app
```

`./package-app.sh --install` also copies the bundle to:

```text
/Applications/Well.app
```

The install step is opt-in.

## Configuration

The settings panel saves JSON config to:

```text
~/.config/well/config.json
```

JSON is the canonical runtime configuration format. Saves use the current
versioned schema and atomically replace the previous file. Legacy unversioned
and version 1 files migrate on load; files from newer unsupported versions are
rejected without changing the running defaults. Invalid or out-of-range values
fall back to safe defaults or are normalized to supported ranges.

API tokens are not written by new saves. They can be supplied through
`GEMINI_API_KEY` or `HF_TOKEN`, or saved from the settings UI to the operating
system credential store. On macOS, Well uses Keychain service
`org.well.terminal.ai` with accounts `gemini-api-key` and
`hugging-face-token`; environment variables take precedence.

The Hyprlang import/export path is:

```text
~/.config/well/well.hl
```

Hyprlang remains an explicit import/export format. It is not read automatically
at startup and does not contain provider credentials.

Named profiles are stored as versioned JSON files under:

```text
~/.config/well/profiles/
```

Profiles use the same credential-free `PersistentConfig` shape as
`config.json`, including shell path, active font, abbreviations, and
keybindings. Older Theia-only profile JSON files still load for migration
compatibility, but newly saved profiles are written in the canonical versioned
format.

## Generated images

Pythia image generation writes provider-returned image artifacts to:

```text
~/.well/generated/
```

Files are content-addressed by provider, prompt, and requested dimensions, so repeated prompts reuse cached output. A manual regenerate request bypasses that cache and refreshes the artifact. Mock PNGs require a test build or the explicit `test-support` Cargo feature as well as `WELL_MOCK_LLM=1`; release builds ignore the environment switch.

## Logs and journals

The desktop app initializes `env_logger`; warnings and errors go to stderr by
default. To increase verbosity when launching from a shell:

```bash
RUST_LOG=well=debug cargo run --bin well
```

Credential-store lookup failures are quiet during startup unless
`WELL_LOG_CREDENTIAL_ERRORS` is set. Explicit save/remove credential actions
still report errors in the settings UI.

The PTY journal currently writes to:

```text
/tmp/well-history.log
```

This path is local machine state and should not be committed.

## Cleanup

Safe local cleanup targets:

```bash
rm -rf dist/Well.app
rm -rf ~/.well/generated
rm -f /tmp/well-history.log
```

Be careful with `~/.config/well/config.json`; deleting it resets saved Well settings.

## GPUI operator settings and services

GPUI saves its operator settings to `~/.config/well/operator_settings.json` and syncs
shared typography through `well-config::ConfigStore`. The older JSON/Hyprlang profile
system described above remains a separate legacy interface; consolidation requires
an explicit migration rather than silently choosing one file over another.

Hermes uses `$XDG_STATE_HOME/well/run/caduceus.sock`, or
`~/.local/state/well/run/caduceus.sock`. The server requires a private parent directory
and matching local peer UID. Remote SSH forwarding retains the explicit remote socket
name while resolving the correct private local endpoint.

The GPUI AgentOS panel exports under `~/.local/state/well/agentos` (or the matching
XDG state location). Its standalone CLI accepts an explicit output directory. Files
include `BOOTX64.EFI`, `agent_os_unikernel.elf`, `agentos-esp.img`, `manifest.json`, and
`SHA256SUMS`. An optional checksummed network bootstrap volume is generated from a
user-supplied full iPXE image. Build caches live below the selected output directory.

Incoming SSH host keys live under `~/.local/state/well/ssh-host`. Authorized clients
come from `~/.ssh/authorized_keys`; password auth is off by default and requires
`WELL_SSH_PASSWORD` in the launching environment. The host does not write that password
to disk. Changes to authentication or authorized keys require restarting the host.

Generated images now have credential-free JSON sidecars containing prompt, provider,
and requested size. History refresh is explicit and regeneration uses that metadata.
Delete only removes managed image/sidecar files within the configured image cache;
it refuses symlinked images and paths outside that cache.

GPUI operator JSON has a version field (currently 1). Existing unversioned files
remain readable. Writes use the shared config layer, atomic temporary files, writer
locking and a content revision check. Future-version, malformed or externally
modified files are preserved and cause a visible save error rather than being overwritten.
