# Well terminal: current application

Run the native terminal from this checkout (macOS or Arch Linux x86_64):

```sh
cargo run --bin well --target-dir /tmp/well-terminal-build
```

Build artifacts stay outside this repository because older build outputs are
tracked here. The window starts one interactive login shell from `$SHELL`
(`/bin/sh` when unset), in the directory where Well was launched. The default
14 pixel font is macOS Menlo or Linux DejaVu Sans Mono, discovered from installed
system fonts. No OS fonts are copied into the repository or distribution.

## Controls

- Type commands normally; the persistent shell supplies editing, history,
  completion, prompt, quoting, pipes, and directory state.
- Use **+**, Cmd+T (macOS), or Ctrl+Shift+T (Linux) for an independent shell tab.
  Cmd+W / Ctrl+Shift+W closes the focused tab and its processes. Ctrl+Tab and
  Ctrl+Shift+Tab cycle tabs. Closing the final tab closes the window.
- Ctrl+C interrupts the foreground program. Ctrl+D sends EOF.
- Cmd+V (macOS) or Ctrl+Shift+V (Linux) pastes text, using bracketed paste when the shell enables it. Escape
  characters are removed from pasted content. Pending input is capped at 1 MiB;
  an overflow reports an error instead of freezing the window.
- Scroll with the wheel or Shift+PageUp/PageDown. Typing returns to live output.
  The scrollback buffer holds up to 10,000 rows.
- Drag to select text, then Cmd+C (macOS) or Ctrl+Shift+C (Linux) to copy. A
  selection holds its visible snapshot while incoming output continues in the
  background. Escape, typing, or scrolling returns to live output. Wrapped rows
  join when copied; wide characters retain their leading cell.
- Applications can request X10, press/release, drag, or all-motion mouse reports
  in default, UTF-8, or SGR encoding. Hold Shift to select or scroll without
  reporting mouse input to the application.
- Resize the window to update both terminal and application dimensions.
- F12, Cmd+, (macOS), Ctrl+, (Linux), or Settings opens preferences. Save & apply
  updates the font, spacing, padding, colors, geometric border, and wheel speed
  and persists the settings atomically. Escape dismisses unsaved changes.
- Cmd+Q (macOS), Ctrl+Shift+Q (Linux), or the close button shuts down the session. Running `exit` leaves the
  window open with the final output and shell exit status.

The renderer supports foreground/background colors, bold, italic, underline,
inverse cells, cursor visibility, and wide-cell layout. Unsupported font glyphs
search installed outlined font faces before using a replacement character. Full-screen programs use the terminal's alternate
screen. Shell output is not automatically written to a temporary history log.

## Remaining release integration

This milestone uses a PTY and the shell's own prompt. Embedded Fish, native IPC
execution, the custom editor/prompt, scrollback search, image
display, complex-script shaping, and color emoji remain future work. The
HTTP/WebSocket RPC transport is implemented and tested as a library; desktop
dispatch now connects real workspace state, tab creation/switching, terminal
reads, and targeted shell input. The private socket appears in **Automation**
and each child shell receives its path and surface ID. See [automation](automation.md).
Split panes, editor insertion, and agent lifecycle UI remain unregistered.

## Validation

```sh
cargo check --workspace --target-dir /tmp/well-terminal-build
cargo test --workspace --target-dir /tmp/well-terminal-build
```

The controlled PTY tests use `/bin/sh` without login startup files and disable
history-file writes. They exercise fragmented queries, alternate screens,
quoting/pipelines, persistent directory state, resizing, interrupts, final output
before exit notification, scrollback, bounded input, Vim with configuration and
viminfo disabled, and shutdown of a foreground job that ignores hangup.
