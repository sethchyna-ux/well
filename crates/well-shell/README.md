# 🦉 well-shell: Metis Shell Supervisor & HyperShell Engine

The `well-shell` crate supervises the interactive terminal session, handles pseudo-terminal (PTY) communication, provides sub-millisecond command autocomplete via prefix tries, and orchestrates asynchronous background pipelines via HyperShell.

---

## Architecture & Modules

### 1. Pseudo-Terminal Host (`pty.rs` / `PtySession`)
* **Shell Spawner**: Spawns interactive login shells, prioritizing `/opt/homebrew/bin/fish` -> `$SHELL` -> `/bin/zsh`.
* **Automated Device Attribute Responder (`handle_terminal_queries`)**:
  * Intercepts VT/ANSI device queries written by modern shells like Fish 4.x.
  * **DA1 (Primary Device Attributes)**: Immediately echoes `\x1b[?62;1;2;6;7;8;9c` to stdin, satisfying Fish 4.x startup queries in `< 0.1ms` and eliminating the 10-second hang.
  * **DA2 (Secondary Device Attributes)**: Echoes `\x1b[>0;10;1c`.
  * **DSR (Device Status Report / Cursor Position)**: Echoes live cursor coordinates `\x1b[{row};{col}R`.
  * **Terminal Status**: Echoes `\x1b[0n`.
* **VT100 Integration**: Feeds raw ANSI escape sequences directly into an internal `vt100::Parser` screen buffer for zero-overhead rendering.

### 2. Metis Autocomplete Core (`lib.rs` / `MetisExecutor`)
* **In-Memory Prefix Trie**: Fast $\mathcal{O}(k)$ autocomplete where $k$ is active query length.
* **Smart Ranking**: Tracks execution frequencies and timestamps for intelligent predictive suggestion.

### 3. HyperShell Engine (`hypershell.rs` / `HyperShellEngine`)
* **Asynchronous Pipelines**: Runs multi-stage pipelines (`cmd1 | cmd2 | cmd3`) with Tokio streams.
* **Background Tasks**: Non-blocking background job scheduling with microsecond-level wall-time telemetry (`TaskResult`).
* **History Feed**: Automatically seeds executed commands into `MetisHistory` for subsequent trie autocomplete.

---

## Testing

```bash
cargo test -p well-shell
```
