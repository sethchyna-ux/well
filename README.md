# Well

Well is a native Rust terminal workspace using winit, wgpu, a persistent system
shell, and a font-backed terminal grid. Release targets are macOS and Arch Linux
x86_64. macOS runs locally; native Arch acceptance remains open.

**Release hardening is in progress.** The [release gate](docs/release-readiness.md)
tracks the remaining work; this checkout is not yet a finished release.

## Run

```sh
cargo run --bin well --target-dir /tmp/well-terminal-build
```

The terminal preserves shell state, supports interactive programs and alternate
screens, handles Unicode input and colored output, and provides paste,
scrollback, resizing, interrupts, persistent appearance settings, and owned session shutdown. See
[terminal controls](docs/well-terminal.md).

The workspace also contains the syntax-aware rope editor, command history
index, structured process transport, prompt formatter, configuration component,
and local HTTP/WebSocket JSON-RPC service. Their application integration and
release status are tracked individually in the release gate.

The desktop supports independent persistent shell tabs and starts a private local
automation socket. Use **+** or Cmd+T / Ctrl+Shift+T for a tab, and **Automation**
for the socket path. Registered RPC methods can inspect the workspace, create
and switch tabs, read terminal text, and queue input to an explicit surface ID.
See [automation and tab controls](docs/automation.md) for the protocol and limits.

## Build and check

```sh
./well-build-harness.sh
```

The harness checks and tests the Cargo workspace and builds the release binary
from the checked-in sources. Build products go outside the checkout. It does
not overwrite source, manifests, or user configuration. Set
`WELL_BUILD_TARGET_DIR` to select another build cache.

For a local macOS application bundle and checksummed archive:

```sh
./well-build-harness.sh --package
```

Bundles are ad-hoc signed unless `WELL_SIGNING_IDENTITY` selects an existing
signing identity. Public distribution, notarization, and the complete release
acceptance gate have not yet been completed.

On Arch Linux x86_64, install `base-devel rust fontconfig ttf-dejavu
libxkbcommon libxkbcommon-x11 libx11 libxcursor libxi libxrandr wayland
vulkan-icd-loader`, plus the Vulkan driver for your GPU. The same `--package`
command uses `makepkg` to create a checksummed `.pkg.tar.zst`; run it as your
normal user. Linux uses installed fonts rather than a macOS font path.

## Appearance

Settings (F12, Cmd+, on macOS, Ctrl+, on Linux) changes the font, font size,
spacing, padding, colors, geometric border, and wheel speed. Save & apply validates
the chosen font and updates the running terminal. Preferences are stored at
`~/Library/Application Support/Well/settings.toml` on macOS and
`${XDG_CONFIG_HOME:-~/.config}/well/settings.toml` on Linux. `WELL_CONFIG_FILE`
selects an explicit file. Invalid files produce an error and are not overwritten;
changes made by another instance require reloading before saving.
