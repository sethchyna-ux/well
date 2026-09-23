//! pty.rs: Pseudo-Terminal (PTY) Subsystem for Well Terminal
//!
//! Provides bidirectional streaming between the OS shell (zsh/bash) and
//! the Orpheus/Atlas rendering pipeline via portable-pty and vt100.

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

pub const DEFAULT_SCROLLBACK_LIMIT: usize = 10_000;

pub struct PtySession {
    pub parser: Arc<Mutex<vt100::Parser>>,
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
    pub graphic_events_rx:
        std::sync::Mutex<std::sync::mpsc::Receiver<well_render::graphics_protocol::GraphicEvent>>,
}

fn normalized_shell_path(value: Option<String>) -> Option<String> {
    let shell = value?.trim().to_string();
    (!shell.is_empty()).then_some(shell)
}

/// Returns whether an explicit shell path points to an executable file.
///
/// Well only uses this check for a saved, user-selected shell path. The normal
/// `$SHELL` fallback remains intentionally permissive for platform compatibility.
pub fn is_executable_shell_path(shell: &str) -> bool {
    let path = std::path::Path::new(shell.trim());
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };

    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

fn user_shell_from_environment() -> Option<String> {
    normalized_shell_path(std::env::var("SHELL").ok())
        .filter(|shell| is_executable_shell_path(shell))
}

fn terminal_parser(rows: u16, cols: u16, scrollback_limit: usize) -> vt100::Parser {
    vt100::Parser::new(rows, cols, scrollback_limit.max(1))
}

/// Inspects incoming terminal output stream from the child process and auto-replies
/// to standard VT/ANSI query escape sequences (DA1 Primary Device Attributes, DA2, DSR, etc.).
///
/// Modern shells like Fish 4.x send DA1 (`\x1b[c` or `\x1b[0c`) upon startup and block for up to
/// 10 seconds if no response is received. Answering immediately makes shell startup instantaneous.
pub(crate) fn handle_terminal_queries<W: Write + ?Sized>(
    data: &[u8],
    parser: &Arc<Mutex<vt100::Parser>>,
    writer: &mut W,
) -> std::io::Result<()> {
    if !data.contains(&0x1b) {
        return Ok(());
    }

    // 1. Primary Device Attributes (DA1): \x1b[c or \x1b[0c
    // Fish specifically requires a CSI sequence starting with '?' and ending with 'c'.
    // Standard VT220 / xterm response: \x1b[?62;1;2;6;7;8;9c or \x1b[?1;2c
    if data.windows(3).any(|w| w == b"\x1b[c") || data.windows(4).any(|w| w == b"\x1b[0c") {
        writer.write_all(b"\x1b[?62;1;2;6;7;8;9c")?;
        writer.flush()?;
    }

    // 2. Secondary Device Attributes (DA2): \x1b[>c or \x1b[>0c
    if data.windows(4).any(|w| w == b"\x1b[>c") || data.windows(5).any(|w| w == b"\x1b[>0c") {
        writer.write_all(b"\x1b[>0;10;1c")?;
        writer.flush()?;
    }

    // 3. Device Status Report (Cursor Position): \x1b[6n
    if data.windows(4).any(|w| w == b"\x1b[6n") {
        let (row, col) = if let Ok(p) = parser.lock() {
            let (r, c) = p.screen().cursor_position();
            (r + 1, c + 1)
        } else {
            (1, 1)
        };
        let resp = format!("\x1b[{};{}R", row, col);
        writer.write_all(resp.as_bytes())?;
        writer.flush()?;
    }

    // 4. Device Status: \x1b[5n -> \x1b[0n (Ready, no malfunction)
    if data.windows(4).any(|w| w == b"\x1b[5n") {
        writer.write_all(b"\x1b[0n")?;
        writer.flush()?;
    }

    Ok(())
}

impl PtySession {
    /// Spawns a new interactive login shell, preferring the user's configured `$SHELL`.
    /// connected to a virtual terminal screen buffer of dimensions (rows, cols).
    pub fn spawn<F>(rows: u16, cols: u16, on_output: F) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: Fn() + Send + 'static,
    {
        Self::spawn_with_shell_and_scrollback(rows, cols, None, DEFAULT_SCROLLBACK_LIMIT, on_output)
    }

    /// Spawns a new interactive shell with an optional custom shell executable path.
    pub fn spawn_with_shell<F>(
        rows: u16,
        cols: u16,
        custom_shell: Option<&str>,
        on_output: F,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: FnMut() + Send + 'static,
    {
        Self::spawn_with_shell_and_scrollback(
            rows,
            cols,
            custom_shell,
            DEFAULT_SCROLLBACK_LIMIT,
            on_output,
        )
    }

    /// Spawns a new interactive shell with a configured scrollback capacity.
    pub fn spawn_with_shell_and_scrollback<F>(
        rows: u16,
        cols: u16,
        custom_shell: Option<&str>,
        scrollback_limit: usize,
        mut on_output: F,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: FnMut() + Send + 'static,
    {
        Self::spawn_with_stream_and_scrollback(
            rows,
            cols,
            custom_shell,
            scrollback_limit,
            move |_| on_output(),
        )
    }

    /// Spawns a new interactive shell with a raw stream callback delivering output bytes.
    pub fn spawn_with_stream<F>(
        rows: u16,
        cols: u16,
        custom_shell: Option<&str>,
        on_stream: F,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: FnMut(&[u8]) + Send + 'static,
    {
        Self::spawn_with_stream_and_scrollback(
            rows,
            cols,
            custom_shell,
            DEFAULT_SCROLLBACK_LIMIT,
            on_stream,
        )
    }

    /// Spawns a new interactive shell with a raw stream callback and explicit
    /// terminal scrollback capacity.
    pub fn spawn_with_stream_and_scrollback<F>(
        rows: u16,
        cols: u16,
        custom_shell: Option<&str>,
        scrollback_limit: usize,
        mut on_stream: F,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: FnMut(&[u8]) + Send + 'static,
    {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = if let Some(s) = custom_shell {
            let shell = normalized_shell_path(Some(s.to_string())).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "configured shell path is empty",
                )
            })?;
            if !is_executable_shell_path(&shell) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("configured shell is not an executable file: {shell}"),
                )
                .into());
            }
            shell
        } else if cfg!(windows) {
            std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string())
        } else if let Some(shell) = user_shell_from_environment() {
            shell
        } else if std::path::Path::new("/opt/homebrew/bin/fish").exists() {
            "/opt/homebrew/bin/fish".to_string()
        } else if std::path::Path::new("/usr/bin/fish").exists() {
            "/usr/bin/fish".to_string()
        } else if std::path::Path::new("/usr/local/bin/fish").exists() {
            "/usr/local/bin/fish".to_string()
        } else {
            "/bin/sh".to_string()
        };

        let mut cmd = CommandBuilder::new(&shell);
        if cfg!(unix) {
            cmd.arg("-l");
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "Well");

        let home_dir = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"));
        if let Ok(home) = home_dir {
            cmd.cwd(home);
        }

        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave); // Vital: closes slave fd in parent so reader EOF functions correctly on shell exit
        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;

        let writer_arc = Arc::new(Mutex::new(writer));
        let writer_clone = Arc::clone(&writer_arc);

        let parser = Arc::new(Mutex::new(terminal_parser(rows, cols, scrollback_limit)));
        let parser_clone = Arc::clone(&parser);

        let (gfx_tx, gfx_rx) = std::sync::mpsc::channel();

        // Setup Chronos Journal
        let mut journal =
            well_history::journal::Journal::new("/tmp/well-history.log", 10 * 1024 * 1024).ok();

        // Background reader thread: captures raw ANSI/VT escape stream, responds to terminal queries,
        // and processes into vt100 screen buffer
        thread::Builder::new()
            .name("well-pty-reader".to_string())
            .spawn(move || {
                let mut buf = [0u8; 65536];
                let mut interceptor = well_render::graphics_protocol::Interceptor::new();
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            let slice = &buf[..n];

                            // Log raw output to Chronos Journal
                            if let Some(j) = journal.as_mut() {
                                let _ = j.append(&well_history::journal::EventRecord::StdoutEvent(
                                    slice.to_vec(),
                                ));
                            }

                            // 1. Intercept graphic protocols (APC Kitty Images, OSC 7/8)
                            let sanitized = interceptor.process(slice);

                            // Send intercepted events to GUI thread
                            while let Some(event) = interceptor.events.pop_front() {
                                let _ = gfx_tx.send(event);
                            }

                            if sanitized.contains(&0x1b) {
                                if let Ok(mut w) = writer_clone.lock() {
                                    let _ = handle_terminal_queries(
                                        &sanitized,
                                        &parser_clone,
                                        &mut **w,
                                    );
                                }
                            }
                            if let Ok(mut p) = parser_clone.lock() {
                                p.process(&sanitized);
                            }
                            on_stream(&sanitized);
                        }
                        Err(_) => break,
                    }
                }
            })?;

        Ok(Self {
            parser,
            writer: writer_arc,
            master: Arc::new(Mutex::new(pair.master)),
            child: Arc::new(Mutex::new(child)),
            graphic_events_rx: std::sync::Mutex::new(gfx_rx),
        })
    }

    /// Sends raw keystrokes or escape sequences to child process stdin
    pub fn write_all(&self, bytes: &[u8]) -> std::io::Result<()> {
        let mut w = self
            .writer
            .lock()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        w.write_all(bytes)?;
        w.flush()?;
        Ok(())
    }

    /// Resizes both the kernel PTY driver and the vt100 virtual grid
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(master) = self.master.lock() {
            master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
        }

        if let Ok(mut parser) = self.parser.lock() {
            parser.screen_mut().set_size(rows, cols);
        }

        Ok(())
    }

    /// Sends the platform terminal's end-of-input control character to the child process.
    pub fn send_eof(&self) -> std::io::Result<()> {
        #[cfg(unix)]
        const EOF_SEQUENCE: &[u8] = b"\x04";
        #[cfg(windows)]
        const EOF_SEQUENCE: &[u8] = b"\x1a";

        self.write_all(EOF_SEQUENCE)
    }

    /// Polls the child process without blocking.
    pub fn try_wait(&self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
        let mut child = self
            .child
            .lock()
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        child.try_wait()
    }

    /// Waits up to `timeout` for the child process to exit.
    pub fn wait_for_exit(
        &self,
        timeout: std::time::Duration,
    ) -> std::io::Result<Option<portable_pty::ExitStatus>> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(Some(status));
            }
            if std::time::Instant::now() >= deadline {
                return Ok(None);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Check if child process is still running
    pub fn is_alive(&self) -> bool {
        self.try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        let Ok(mut child) = self.child.lock() else {
            return;
        };
        if matches!(child.try_wait(), Ok(None)) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_scrollback_capacity_is_applied_to_terminal_parser() {
        let mut parser = terminal_parser(2, 20, 3);
        for line in 0..12 {
            parser.process(format!("line-{line}\r\n").as_bytes());
        }

        parser.screen_mut().set_scrollback(usize::MAX);
        assert_eq!(parser.screen().scrollback(), 3);
    }

    #[test]
    fn normalized_shell_path_ignores_empty_environment_values() {
        assert_eq!(normalized_shell_path(None), None);
        assert_eq!(normalized_shell_path(Some("  \t".to_string())), None);
        assert_eq!(
            normalized_shell_path(Some(" /opt/homebrew/bin/fish ".to_string())),
            Some("/opt/homebrew/bin/fish".to_string())
        );
    }

    #[cfg(unix)]
    #[test]
    fn executable_shell_path_accepts_system_shell_and_rejects_missing_path() {
        assert!(is_executable_shell_path("/bin/sh"));
        assert!(!is_executable_shell_path("/well/does-not-exist"));
    }

    #[cfg(unix)]
    #[test]
    fn test_pty_session_lifecycle_write_resize_eof_and_exit() {
        use std::sync::mpsc::{self, Receiver};
        use std::time::{Duration, Instant};

        fn receive_until(receiver: &Receiver<Vec<u8>>, expected: &str) -> String {
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut output = Vec::new();
            while Instant::now() < deadline {
                let remaining = deadline.saturating_duration_since(Instant::now());
                match receiver.recv_timeout(remaining) {
                    Ok(chunk) => {
                        output.extend_from_slice(&chunk);
                        if String::from_utf8_lossy(&output).contains(expected) {
                            return String::from_utf8_lossy(&output).into_owned();
                        }
                    }
                    Err(_) => break,
                }
            }
            panic!(
                "timed out waiting for {expected:?}; output was {:?}",
                String::from_utf8_lossy(&output)
            );
        }

        let (output_tx, output_rx) = mpsc::channel();
        let session = PtySession::spawn_with_stream(24, 80, Some("/bin/sh"), move |chunk| {
            let _ = output_tx.send(chunk.to_vec());
        })
        .expect("failed to spawn test PTY");

        assert!(session.is_alive());
        session
            .write_all(b"printf '__well_%s__\\n' 'pty_write'\n")
            .expect("failed to write command to PTY");
        receive_until(&output_rx, "__well_pty_write__");

        session.resize(31, 91).expect("failed to resize PTY");
        assert_eq!(session.parser.lock().unwrap().screen().size(), (31, 91));
        session
            .write_all(b"stty size\n")
            .expect("failed to query PTY size");
        receive_until(&output_rx, "31 91");

        session.send_eof().expect("failed to send PTY EOF");
        let status = session
            .wait_for_exit(Duration::from_secs(3))
            .expect("failed to poll child after EOF")
            .expect("shell did not exit after EOF");
        assert!(status.success());
        assert!(!session.is_alive());

        let exiting_session = PtySession::spawn_with_stream(24, 80, Some("/bin/sh"), |_| {})
            .expect("failed to spawn exit-status PTY");
        exiting_session
            .write_all(b"exit 7\n")
            .expect("failed to write exit command");
        let status = exiting_session
            .wait_for_exit(Duration::from_secs(3))
            .expect("failed to poll explicit child exit")
            .expect("shell did not process exit command");
        assert_eq!(status.exit_code(), 7);
    }

    #[cfg(unix)]
    #[test]
    fn test_pty_rapid_resize_and_output_stress() {
        use std::sync::mpsc;
        use std::time::{Duration, Instant};

        let (output_tx, output_rx) = mpsc::channel();
        let session = PtySession::spawn_with_stream(24, 80, Some("/bin/sh"), move |chunk| {
            let _ = output_tx.send(chunk.to_vec());
        })
        .expect("failed to spawn PTY stress shell");

        session
            .write_all(b"stty -echo\n")
            .expect("failed to disable PTY echo");
        std::thread::sleep(Duration::from_millis(100));
        while output_rx.try_recv().is_ok() {}

        let mut final_size = (24, 80);
        for iteration in 0..250 {
            final_size = (20 + iteration % 41, 60 + iteration % 101);
            session
                .resize(final_size.0, final_size.1)
                .expect("rapid PTY resize failed");
        }
        assert_eq!(session.parser.lock().unwrap().screen().size(), final_size);

        session
            .write_all(
                b"stty size; i=0; while [ $i -lt 2000 ]; do printf 'well-stress-%04d\\n' \"$i\"; i=$((i + 1)); done; printf '__well_stress_complete__\\n'\n",
            )
            .expect("failed to write PTY stress workload");

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut output = Vec::new();
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let Ok(chunk) = output_rx.recv_timeout(remaining) else {
                break;
            };
            output.extend_from_slice(&chunk);
            if output
                .windows(b"__well_stress_complete__".len())
                .any(|window| window == b"__well_stress_complete__")
            {
                break;
            }
        }

        let output = String::from_utf8_lossy(&output);
        let mut output_tail = output.chars().rev().take(500).collect::<Vec<_>>();
        output_tail.reverse();
        let output_tail = output_tail.into_iter().collect::<String>();
        assert!(
            output.contains(&format!("{} {}", final_size.0, final_size.1)),
            "shell did not observe final PTY size {final_size:?}; output tail: {:?}",
            output_tail
        );
        assert!(output.contains("well-stress-1999"));
        assert!(output.contains("__well_stress_complete__"));

        session.send_eof().expect("failed to send PTY stress EOF");
        let status = session
            .wait_for_exit(Duration::from_secs(3))
            .expect("failed to wait for PTY stress shell")
            .expect("PTY stress shell did not exit");
        assert!(status.success());
    }

    #[test]
    fn test_terminal_query_primary_device_attributes() {
        let parser = Arc::new(Mutex::new(vt100::Parser::new(24, 80, 100)));
        let mut out = Vec::new();

        handle_terminal_queries(b"\x1b[c", &parser, &mut out).unwrap();
        assert_eq!(out, b"\x1b[?62;1;2;6;7;8;9c");

        out.clear();
        handle_terminal_queries(b"\x1b[0c", &parser, &mut out).unwrap();
        assert_eq!(out, b"\x1b[?62;1;2;6;7;8;9c");
    }

    #[test]
    fn test_terminal_query_secondary_device_attributes() {
        let parser = Arc::new(Mutex::new(vt100::Parser::new(24, 80, 100)));
        let mut out = Vec::new();

        handle_terminal_queries(b"\x1b[>c", &parser, &mut out).unwrap();
        assert_eq!(out, b"\x1b[>0;10;1c");

        out.clear();
        handle_terminal_queries(b"\x1b[>0c", &parser, &mut out).unwrap();
        assert_eq!(out, b"\x1b[>0;10;1c");
    }

    #[test]
    fn test_terminal_query_device_status_ready() {
        let parser = Arc::new(Mutex::new(vt100::Parser::new(24, 80, 100)));
        let mut out = Vec::new();

        handle_terminal_queries(b"\x1b[5n", &parser, &mut out).unwrap();
        assert_eq!(out, b"\x1b[0n");
    }

    #[test]
    fn test_terminal_query_cursor_position_report() {
        let parser = Arc::new(Mutex::new(vt100::Parser::new(24, 80, 100)));
        {
            let mut parser = parser.lock().unwrap();
            parser.process(b"\x1b[4;7H");
        }
        let mut out = Vec::new();

        handle_terminal_queries(b"\x1b[6n", &parser, &mut out).unwrap();
        assert_eq!(out, b"\x1b[4;7R");
    }

    #[test]
    fn test_terminal_query_ignores_plain_output() {
        let parser = Arc::new(Mutex::new(vt100::Parser::new(24, 80, 100)));
        let mut out = Vec::new();

        handle_terminal_queries(b"plain shell output", &parser, &mut out).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn test_vt100_parser_resize_updates_grid_size() {
        let mut parser = vt100::Parser::new(24, 80, 100);
        assert_eq!(parser.screen().size(), (24, 80));

        parser.screen_mut().set_size(40, 120);
        assert_eq!(parser.screen().size(), (40, 120));
    }
}
