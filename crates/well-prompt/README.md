# ⭐ well-prompt: Astraea Sub-100µs Zero-Fork Prompt Engine

The `well-prompt` crate implements a high-speed prompt vector compiler designed to eliminate shell latency bottlenecks.

---

## Architectural Purpose

Traditional shell prompts execute external processes (`git status`, `hostname`, `date`, `whoami`) on every newline, consuming anywhere from 30ms to 250ms of CPU time and battery power.

**Astraea** keeps prompt rendering independent from shell startup scripts:

* Maintains an in-memory `AstraeaStateVector`.
* Refreshes Git prompt state without spawning `git`: branch, detached HEAD,
  packed refs, linked worktree gitdirs, in-progress operations such as merge or
  rebase, and a deliberately limited dirty signal based on index lock files.
* Compiles the full styled prompt vector in **under 100 microseconds** ($\mathcal{O}(1)$ execution).
* Fully compatible with transient prompt workflows where previous prompts collapse to compact indicators upon command execution.
* Is exposed through FFI standalone prompt rendering; the desktop PTY still uses the user's configured shell prompt.

Astraea does not run `git status` in the hot path. Worktree dirtiness is
therefore represented with explicit confidence instead of pretending to have a
complete file-status scan.

---

## Testing

```bash
cargo test -p well-prompt
```
