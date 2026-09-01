# ⭐ well-prompt: Astraea Sub-100µs Zero-Fork Prompt Engine

The `well-prompt` crate implements a high-speed prompt vector compiler designed to eliminate shell latency bottlenecks.

---

## Architectural Purpose

Traditional shell prompts execute external processes (`git status`, `hostname`, `date`, `whoami`) on every newline, consuming anywhere from 30ms to 250ms of CPU time and battery power.

**Astraea** bypasses process forks entirely:
* Maintains an in-memory `AstraeaStateVector` updated asynchronously via background file watchers and OS notifications.
* Compiles the full styled prompt vector in **under 100 microseconds** ($\mathcal{O}(1)$ execution).
* Fully compatible with transient prompt workflows where previous prompts collapse to compact indicators upon command execution.

---

## Testing

```bash
cargo test -p well-prompt
```
