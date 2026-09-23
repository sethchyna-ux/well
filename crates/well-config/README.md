# 🪟 well-config: Theia Control Center & Hyprlang Engine

The `well-config` crate contains versioned JSON configuration, profile storage,
the egui-powered Theia Control Center interface, and a native parser/emitter
for the Wayland-inspired Hyprlang block format (`.hl`). JSON is the canonical
runtime format; Hyprlang is an explicit import/export format.

---

## Features

### 1. Theia Control Center (`TheiasPrismPanel`)

* **Access**: Toggle anytime with `Cmd+,`, `Ctrl+,`, `F12`, `F1`, or the top-right floating glass pill.
* **Dismiss**: Instant closing via `Esc` key, window header `✖`, or the bottom action bar `[ ✕ Close (Esc) ]` button.
* **Smooth Interaction**: Fully movable and resizable window with 60 FPS drag repainting and real-time Hermes Seqlock telemetry.
* **Segmented Navigation**:
  1. *Appearance*: Theme selection (Cyber-Neon, Tokyo Night, Matrix Green, Synthwave '84), background opacity, and cursor styling.
  2. *Typography*: Font size slider, line height, cursor styles (Block, Beam, Underline), cursor blinking.
  3. *Shortcuts*: Runtime-backed keybinding actions for settings, search, command palette, selection, clear, and font sizing.
  4. *Shell*: Executable shell path, startup scrollback capacity, and Starship preset application.
  5. *Profiles & Hyprlang*: Profile create/load/delete, JSON persistence, and one-click Hyprlang import/export.
  6. *Help*: Current capabilities, live shortcut status, and deferred feature boundaries.
  7. *Image Studio*: Provider-backed image generation with local artifact history, preview, open/reveal/copy/insert/regenerate/delete actions, and explicit credential/provider status.
* **Undo & Redo**: Stack-based state history allows reverting or re-applying any visual changes.

### 2. Native Hyprlang Engine (`hyprlang.rs`)

* **`HyprlangParser`**: Parses `.hl` files into `TheiaConfigPayload` structures and keybindings.
* **`HyprlangEmitter`**: Serializes active configuration and hotkeys into clean, human-readable Hyprlang blocks.

```ini
appearance {
    theme_id = 0
    background_opacity = 0.95
    font_size = 14.0
}
bind = Cmd, Shift+P, Open Command Palette
```

---

## Testing

```bash
cargo test -p well-config
```

The canonical file is `~/.config/well/config.json`. Version 2 writes are
atomic, omit provider credentials, migrate unversioned/version 1 files, and
reject unsupported future versions without replacing safe runtime defaults.
Provider credentials can instead be resolved from environment variables or the
OS credential store and are scrubbed from Hermes shared configuration state.
