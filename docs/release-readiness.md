# Well release readiness

Status: **not release ready**. This is the release gate for the active goal,
"bring well to release rdy no placeholders". Passing an individual test suite
or finishing the first usable terminal does not satisfy this gate.

The confirmed release targets are macOS and Arch Linux x86_64. Existing
local source changes are the baseline. Build artifacts are kept outside the
checkout because historical target outputs are tracked.

## Required release gates

- [ ] All shipped functionality has real implementations and observable effects;
  no fabricated telemetry, success responses, UI toggles, or rendering paths.
- [x] Memory-safe concurrent state and owning bounded transport, including
  multiple writers/readers, disconnect, cancellation, and drop behavior.
- [ ] Correct process ownership and I/O ordering under output floods, full input
  queues, interrupts, shell exit, and application shutdown.
- [ ] Functional shell execution and history services; deterministic suggestions;
  quoting, pipelines, redirections, stateful builtins, jobs, and failures covered.
- [ ] Functional inline rope editor with correct UTF-8 incremental parsing,
  selection, editing, syntax feedback, and diagnostics integration.
- [ ] Persisted, validated configuration and a working configuration UI whose
  controls update the corresponding running subsystem.
- [ ] Actual prompt state acquisition and integration, without invented timing or
  branch values; formatting and transience verified.
- [ ] Original native-engine/shared-memory design requirements are implemented
  or explicitly resolved. A working PTY compatibility path does not prove
  embedded Fish/Starship, memory-mapped state, or cross-process zero-copy IPC.
- [ ] A single real renderer with font discovery/fallback, Unicode text behavior,
  styles, input composition, selection/copy, search, mouse modes, and recovery.
- [ ] Secure local HTTP JSON-RPC and WebSocket transport; real UI dispatch,
  truthful capability discovery, authenticated peers, bounded requests,
  timeouts, cancellation, and socket ownership/cleanup.
- [ ] Workspace/surface management and agent lifecycle UI, as exposed by the API.
- [ ] A non-destructive reproducible build/release script, accurate metadata and
  documentation, license/dependency audit, packaged application, and CI gates.
- [ ] Release build, automated tests, protocol regression checks, and native
  end-to-end verification pass for the supported platform(s).
- [ ] Any claimed latency/memory/performance properties are measured on the
  release build; unmeasured properties are not presented as guarantees.

## Verified work (2026-10-20)

The first terminal milestone is implemented and documented in `well-terminal.md`.
A previous checkpoint passed 54 tests, including:

- Bounded MPMC delivery with concurrent producers/consumers, exact ownership and
  value destruction, async backpressure, and disconnect wakeups.
- Synchronized owned application state with multiwriter/read consistency and
  nonblocking render reads; no hand-written payload seqlocks remain.
- Native stdout/stderr completion before exit status, process-group cancellation,
  explicit child reaping, and shutdown under PTY output floods and full queues.
- A real persistent asynchronous shell service, including directory persistence
  and cancellation while its output receiver is full.
- UTF-8-correct incremental rope edits compared with fresh Tree-sitter parses,
  multi-cursor selection, undo/redo, stale diagnostics rejection, and syntax
  highlights from the Bash grammar.
- Compressed command-history indexing and deterministic frequency/recency ranking,
  including an actual 260,000-command indexing test and long-command handling.
- Local HTTP and WebSocket JSON-RPC, fragmented HTTP bodies, batches, notifications,
  parameter validation, dispatcher timeout, private sockets, real peer UID checks,
  and preservation of unrelated/replacement files during socket cleanup.

The duplicate geometric stand-in renderer now delegates to the canonical
font-backed renderer. The build harness now builds checked-in sources and can
assemble an ad-hoc signed macOS bundle plus a checksummed archive. A debug bundle
was assembled and its signature verified; source and manifest hashes were
unchanged by that packaging run. The legacy source-generating build script is
removed. An empty `.gitignore` directory was replaced by working ignore rules.

`cargo-audit` found zero known vulnerabilities in the current lockfile, with
maintenance advisories for `paste` (RUSTSEC-2024-0436), `serial`
(RUSTSEC-2017-0008), and `ttf-parser` (RUSTSEC-2026-0192). These require tracking
when choosing the final graphics/PTY dependencies; this is not a claim that the
full dependency release gate is complete.

## Known remaining implementation work

The fabricated configuration dashboard has been replaced with validated, atomic
TOML preferences and a working native settings editor. Font family/size, spacing,
padding, colors, geometry, and wheel speed affect the running terminal. The
compact cyan/violet header and geometric border follow the user's reference.
System font discovery replaces the fixed macOS font path, and missing outlined
glyphs search installed font faces. Complex shaping and color emoji remain open.

Native macOS build and workspace/all-target checks pass after this integration.
The settings preview saved and applied a border change to an isolated configuration
file. The 54-test checkpoint above predates this UI integration; tests have not
been rerun for it. Native CI builds and Arch packaging are defined, but the Arch
job has not run and no local Linux VM/container engine is available.

Selection/copy now uses a detached visible snapshot, preserves wrapped rows and
wide-cell text, and resumes live output on Escape/input/scroll. X10, press/release,
button-motion, and all-motion mouse reports support default/UTF-8/SGR encoding,
with Shift override. IME preedit is visible and candidate placement follows the
terminal cursor. Their native regression acceptance remains open.

The desktop now owns independent persistent tabs, stable surface IDs, background
output coalescing, per-tab exit status, all-tab resize/settings updates, and
asynchronous tab cleanup. The local HTTP/WebSocket service starts with the app
and dispatches workspace state, new/switch tab, terminal reads, and targeted
shell input on the UI thread. Cancelled requests are skipped before execution,
dispatch work per event is bounded, and acknowledgements report queued input
bytes rather than command completion. Capabilities contain only connected methods.

A native macOS preview verified two shells with independent working directories
(`/tmp` and `/`), targeted input and actual output reads, tab switching, a real
exit status of 7, rejection of exited/missing targets and invalid tab indices,
and unavailable split methods. Socket directory/socket modes were 0700/0600;
shutdown removed the owned socket. Native tab rendering was inspected. This is
not full cross-platform regression acceptance; keyboard shortcuts and tab cleanup
under stress still need their final acceptance pass.

UI integration of the editor, splits and agent events,
scrollback search, richer text rendering, live prompt acquisition, durable
history, and final release acceptance remain unfinished. The legacy synchronous
executor and batch scheduler also need their full behavior audited.

The RPC transport's earlier library tests predate desktop integration. Native
API checks now cover the connected desktop operations, but the automated suite
has not been rerun for the new integration. The inline editor's model is real,
but its composition UI and actual
language-server transport are not yet connected.

No distribution license has been identified in the checkout; the user's choice
is pending. CI execution, optimized
release validation, measured performance, public-distribution signing/notarization,
and final native end-to-end acceptance are not complete. These are release
blockers, not accepted reductions of the goal.
