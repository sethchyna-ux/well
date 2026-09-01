# 🪟 well-config: Theia Control Center & Hyprlang Engine

The `well-config` crate contains configuration data structures, profile storage, the egui-powered Theia Control Center interface, and a native parser/emitter for the Wayland-inspired Hyprlang block format (`.hl`).

---

## Features

### 1. Theia Control Center (`TheiasPrismPanel`)
* **Access**: Toggle anytime with `Cmd+,`, `Ctrl+,`, `F12`, `F1`, or the top-right floating glass pill.
* **Dismiss**: Instant closing via `Esc` key, window header `✖`, or the bottom action bar `[ ✕ Close (Esc) ]` button.
* **Smooth Interaction**: Fully movable and resizable window with 60 FPS drag repainting and real-time Hermes Seqlock telemetry.
* **Segmented Navigation**:
  1. *Appearance*: Theme selection (Cyber-Neon, Tokyo Night, Matrix Green, Synthwave '84), background opacity, glass blur radius.
  2. *Typography*: Font size slider, line height, cursor styles (Block, Beam, Underline), cursor blinking.
  3. *Shortcuts*: Configurable keybinding actions (splits, clear, font adjustments).
  4. *Metis Shell*: Scan timeout, command timeout, transient prompt toggle.
  5. *CRT Shaders*: Live curvature, scanline frequency, and glow radius sliders with presets.
  6. *Profiles & Hyprlang*: Profile switching, JSON persistence, and one-click Hyprlang import/export.
  7. *Scripts & Engine*: Lua/Wasm scripting engine hooks.
  8. *Help & Docs*: Architecture overview, keyboard cheatsheet, and diagnostics.
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
bind = Cmd, D, Split Pane Horizontal
```

---

## Testing

```bash
cargo test -p well-config
```
