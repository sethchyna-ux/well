# Terminal compatibility

Well's PTY layer has automated lifecycle and terminal-query tests in the normal
workspace test suite. A separate local smoke suite launches common full-screen
programs through the same `PtySession` and `vt100` parser used by the desktop
application.

Run it on macOS with:

```sh
./scripts/smoke-shell-startup.sh
./scripts/smoke-interactive-programs.sh
```

`smoke-shell-startup.sh` probes available local shells (`$SHELL`, `/bin/sh`,
`/bin/zsh`, and common Homebrew/system fish paths). For each installed shell it
verifies startup, marker command execution, PTY resize visibility via
`stty size`, and clean exit.

`smoke-interactive-programs.sh` requires `vim`, `less`, `ssh`, `tmux`, and
`top`. It verifies that:

- Vim and less enter and leave the alternate screen.
- OpenSSH starts in the PTY and reports its client version without requiring a
  network service.
- tmux starts a nested terminal session and returns control cleanly.
- top produces a live process display and accepts its quit command.
- The parent shell remains usable after every program and exits cleanly.

This suite is intentionally opt-in because the required system programs are not
portable Cargo dependencies. Missing programs are reported before the test
starts instead of being silently skipped.
