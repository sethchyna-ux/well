# Local automation

Well starts an HTTP JSON-RPC 2.0 and WebSocket service on a Unix socket. It does
not open a TCP listener. The socket directory is private, the socket is mode
0600, and connections must come from the same OS user. Same-user clients can
read terminal output and send shell input; this is an application control API.

Open **Automation** in the header to copy the socket path and see the actual
connection count. Each shell receives `WELL_RPC_SOCKET` and `WELL_SURFACE_ID`.
The default socket directory is temporary and belongs to this Well instance.
To choose a path at launch, set `WELL_RPC_SOCKET`; its parent must be a private,
owned directory. Existing unrelated files and live sockets are not replaced.
The socket is removed when Well shuts down.

## Calling the API

From a shell inside Well:

```sh
curl --unix-socket "$WELL_RPC_SOCKET" http://localhost/rpc \
  -H 'Content-Type: application/json' \
  --data-binary '{"jsonrpc":"2.0","method":"workspace.state","id":1}'
```

`POST /` also accepts HTTP requests. WebSocket clients connect to `/rpc` over
the Unix stream and send JSON text frames. Requests are limited to 64 KiB,
batches to 64 requests, and concurrent socket connections to 32. Application
requests time out after five seconds. Responses are returned only for requests
with an ID; notifications have no response.

## Implemented methods

| Method | Parameters | Result |
| --- | --- | --- |
| `system.ping` | none | `status: "pong"` |
| `system.capabilities` | none | version and registered methods |
| `workspace.state` | none | active tab index, active surface ID, and each tab's ID, dimensions, and exit status |
| `workspace.new_tab` | none | creates and focuses a persistent shell; returns the resulting workspace state |
| `workspace.switch_tab` | `index`: zero-based integer | switches an existing tab; returns workspace state |
| `surface.send_text` | `text`: string, optional `surface_id`: integer | surface ID and byte count accepted into the PTY input queue |
| `surface.read_text` | optional `surface_id`: integer | plain text from the terminal grid, dimensions, cursor, scrollback offset, and truncation status |

Surface IDs remain stable while tabs are open and are not reused in an instance.
Without a surface ID, surface methods address the currently focused tab. Prefer
an explicit ID when several clients or tabs are active. Read `workspace.state`
to discover IDs, or use `WELL_SURFACE_ID` for the shell making the call.

`surface.send_text` sends exactly the supplied UTF-8 bytes, including control
characters. Its acknowledgement means input was queued; it does not mean a
command finished successfully. Use `\r` to submit a command. Input to an exited
or nonexistent surface returns an error. A tab retains its final output and
exit status until closed.

`surface.read_text` reads the parser's grid at its current scrollback offset.
It includes alternate-screen content, contains no ANSI styling, and is bounded
to 64 KiB of UTF-8 text. `truncated` tells clients when the text was shortened.
A frontend text selection can hold an older snapshot while the parser continues
receiving output; the API continues reading the parser in that case.

The desktop currently registers the methods above. Split panes, editor insertion,
and agent lifecycle operations remain unregistered until their desktop paths are
implemented. An unavailable method returns a JSON-RPC error.

## Tabs

Use the header's **+** button or Cmd+T (macOS), Ctrl+Shift+T (Linux). Cmd+W /
Ctrl+Shift+W closes the focused tab and stops its owned shell and foreground
processes. Closing the final tab closes the window. Ctrl+Tab and Ctrl+Shift+Tab
cycle tabs; Cmd+1–9 / Ctrl+Shift+1–9 selects the corresponding tab index.

Each tab starts a login shell in the directory where Well was launched. Shell
state, working directory, scrollback, output, and exit status are independent.
The limit is 16 tabs. Background output continues to update its own parser;
only the visible tab schedules output redraws. Closing a tab reaps its process
on a shutdown worker, and application exit joins those workers.
