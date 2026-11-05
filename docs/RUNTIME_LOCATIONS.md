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

Files are content-addressed by provider, prompt, and requested dimensions, so repeated prompts reuse cached output. A manual regenerate request bypasses that cache and refreshes the artifact. Test-only mock PNGs are created only when `WELL_MOCK_LLM=1` is explicitly enabled.

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
