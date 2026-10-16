//! pty.rs: Pseudo-Terminal (PTY) Subsystem for Well Terminal
//!
//! Provides bidirectional streaming between the OS shell (fish/zsh/bash) and
//! the rendering pipeline via portable-pty and vt100.
//! Automatically answers VT device attribute queries (DA1, DA2, DSR, Kitty, OSC)
//! and strips query escapes so they never leak into host terminals.

use crate::graphics::{GraphicEvent, Interceptor};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct PtySession {
    pub parser: Arc<Mutex<vt100::Parser>>,
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
    pub graphic_events_rx: Mutex<Receiver<GraphicEvent>>,
}

/// Inspects incoming terminal output stream from the child process, auto-replies
/// to standard VT/ANSI query escape sequences into the child PTY stdin, and returns
/// a sanitized byte vector with the query sequences stripped so they never leak to
/// the host terminal.
pub fn filter_and_respond_queries<W: Write + ?Sized>(
    data: &[u8],
    parser: &Arc<Mutex<vt100::Parser>>,
    writer: &mut W,
) -> Vec<u8> {
    if !data.contains(&0x1b) {
        return data.to_vec();
    }

    let mut sanitized = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        if data[i] == 0x1b {
            let rem = &data[i..];

            // 1. DCS XTGETTCAP Queries & Responses: \x1bP+q<hex>\x1b\ or \x1bP0+q...
            if rem.starts_with(b"\x1bP+q") {
                if let Some(pos) = rem.windows(2).position(|w| w == b"\x1b\\") {
                    let cap_hex = &rem[4..pos];
                    let _ = writer.write_all(b"\x1bP0+q");
                    let _ = writer.write_all(cap_hex);
                    let _ = writer.write_all(b"\x1b\\");
                    let _ = writer.flush();
                    i += pos + 2;
                    continue;
                } else if let Some(pos) = rem.iter().position(|&b| b == 0x07) {
                    let cap_hex = &rem[4..pos];
                    let _ = writer.write_all(b"\x1bP0+q");
                    let _ = writer.write_all(cap_hex);
                    let _ = writer.write_all(b"\x1b\\");
                    let _ = writer.flush();
                    i += pos + 1;
                    continue;
                }
            }
            if rem.starts_with(b"\x1bP0+q") || rem.starts_with(b"\x1bP1+q") {
                if let Some(pos) = rem.windows(2).position(|w| w == b"\x1b\\") {
                    i += pos + 2;
                    continue;
                } else if let Some(pos) = rem.iter().position(|&b| b == 0x07) {
                    i += pos + 1;
                    continue;
                }
            }

            // 2. Alternate Screen Buffer switches during probe handshakes
            if rem.starts_with(b"\x1b[?1049h") || rem.starts_with(b"\x1b[?1049l") {
                i += 8;
                continue;
            }
            if rem.starts_with(b"\x1b[?47h") || rem.starts_with(b"\x1b[?47l") {
                i += 6;
                continue;
            }

            // 3. Primary Device Attributes (DA1): \x1b[c or \x1b[0c
            if rem.starts_with(b"\x1b[c") {
                let _ = writer.write_all(b"\x1b[?62;1;2;6;7;8;9c");
                let _ = writer.flush();
                i += 3;
                continue;
            }
            if rem.starts_with(b"\x1b[0c") {
                let _ = writer.write_all(b"\x1b[?62;1;2;6;7;8;9c");
                let _ = writer.flush();
                i += 4;
                continue;
            }
            // Strip echoed DA1 responses
            if rem.starts_with(b"\x1b[?62;") {
                if let Some(pos) = rem.iter().position(|&b| b == b'c') {
                    i += pos + 1;
                    continue;
                }
            }
            if rem.starts_with(b"\x1b[?1;2c") {
                i += 7;
                continue;
            }

            // 4. Secondary Device Attributes (DA2): \x1b[>c or \x1b[>0c
            if rem.starts_with(b"\x1b[>c") {
                let _ = writer.write_all(b"\x1b[>0;10;1c");
                let _ = writer.flush();
                i += 4;
                continue;
            }
            if rem.starts_with(b"\x1b[>0c") {
                let _ = writer.write_all(b"\x1b[>0;10;1c");
                let _ = writer.flush();
                i += 5;
                continue;
            }
            if rem.starts_with(b"\x1b[>0;10;1c") {
                i += 10;
                continue;
            }

            // 5. Xterm version: \x1b[>q or \x1b[>0q
            if rem.starts_with(b"\x1b[>q") {
                let _ = writer.write_all(b"\x1b[>0;10;1c");
                let _ = writer.flush();
                i += 4;
                continue;
            }
            if rem.starts_with(b"\x1b[>0q") {
                let _ = writer.write_all(b"\x1b[>0;10;1c");
                let _ = writer.flush();
                i += 5;
                continue;
            }
            if rem.starts_with(b"\x1b[>|") {
                if let Some(pos) = rem.windows(2).position(|w| w == b"\x1b\\") {
                    i += pos + 2;
                    continue;
                } else if let Some(pos) = rem.iter().position(|&b| b == 0x07) {
                    i += pos + 1;
                    continue;
                }
            }

            // 6. Cursor position report request: \x1b[6n
            if rem.starts_with(b"\x1b[6n") {
                let (row, col) = if let Ok(p) = parser.lock() {
                    let (r, c) = p.screen().cursor_position();
                    (r + 1, c + 1)
                } else {
                    (1, 1)
                };
                let resp = format!("\x1b[{};{}R", row, col);
                let _ = writer.write_all(resp.as_bytes());
                let _ = writer.flush();
                i += 4;
                continue;
            }

            // 7. Device status request: \x1b[5n, \x1b[?996n, etc.
            if rem.starts_with(b"\x1b[5n") {
                let _ = writer.write_all(b"\x1b[0n");
                let _ = writer.flush();
                i += 4;
                continue;
            }
            if rem.starts_with(b"\x1b[?996n") {
                let _ = writer.write_all(b"\x1b[?996;0n");
                let _ = writer.flush();
                i += 7;
                continue;
            }
            if rem.starts_with(b"\x1b[0n") {
                i += 4;
                continue;
            }

            // 8. Kitty keyboard protocol query: \x1b[?u
            if rem.starts_with(b"\x1b[?u") {
                let _ = writer.write_all(b"\x1b[?0u");
                let _ = writer.flush();
                i += 4;
                continue;
            }
            // Strip echoed or reply \x1b[?0u
            if rem.starts_with(b"\x1b[?0u") {
                i += 5;
                continue;
            }

            // 9. OSC 10 / OSC 11 color queries and echoed responses
            if rem.starts_with(b"\x1b]10;?") || rem.starts_with(b"\x1b]11;?") {
                let is_10 = rem.starts_with(b"\x1b]10;?");
                if rem.starts_with(b"\x1b]10;?\x07") || rem.starts_with(b"\x1b]11;?\x07") {
                    if is_10 {
                        let _ = writer.write_all(b"\x1b]10;rgb:d0d0/d0d0/d0d0\x07");
                    } else {
                        let _ = writer.write_all(b"\x1b]11;rgb:1010/1414/1a1a\x07");
                    }
                    let _ = writer.flush();
                    i += 7;
                    continue;
                } else if rem.starts_with(b"\x1b]10;?\x1b\\") || rem.starts_with(b"\x1b]11;?\x1b\\") {
                    if is_10 {
                        let _ = writer.write_all(b"\x1b]10;rgb:d0d0/d0d0/d0d0\x1b\\");
                    } else {
                        let _ = writer.write_all(b"\x1b]11;rgb:1010/1414/1a1a\x1b\\");
                    }
                    let _ = writer.flush();
                    i += 8;
                    continue;
                }
            }
            if rem.starts_with(b"\x1b]10;rgb:") || rem.starts_with(b"\x1b]11;rgb:") {
                if let Some(pos) = rem.windows(2).position(|w| w == b"\x1b\\") {
                    i += pos + 2;
                    continue;
                } else if let Some(pos) = rem.iter().position(|&b| b == 0x07) {
                    i += pos + 1;
                    continue;
                }
            }
        }

        sanitized.push(data[i]);
        i += 1;
    }

    sanitized
}

pub fn handle_terminal_queries<W: Write + ?Sized>(
    data: &[u8],
    parser: &Arc<Mutex<vt100::Parser>>,
    writer: &mut W,
) {
    let _ = filter_and_respond_queries(data, parser, writer);
}

impl PtySession {
    /// Spawns a new interactive login shell (defaulting to /opt/homebrew/bin/fish, $SHELL, or /bin/zsh)
    /// connected to a virtual terminal screen buffer of dimensions (rows, cols).
    pub fn spawn<F>(rows: u16, cols: u16, on_output: F) -> Result<Self, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn() + Send + 'static,
    {
        Self::spawn_with_shell(rows, cols, None, on_output)
    }

    /// Spawns a new interactive shell with an optional custom shell executable path.
    pub fn spawn_with_shell<F>(
        rows: u16,
        cols: u16,
        custom_shell: Option<&str>,
        mut on_output: F,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnMut() + Send + 'static,
    {
        Self::spawn_with_stream(rows, cols, custom_shell, move |_| on_output())
    }

    /// Spawns a new interactive shell with a raw stream callback delivering output bytes.
    pub fn spawn_with_stream<F>(
        rows: u16,
        cols: u16,
        custom_shell: Option<&str>,
        mut on_stream: F,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>>
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
            s.to_string()
        } else if cfg!(windows) {
            std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string())
        } else if std::path::Path::new("/opt/homebrew/bin/fish").exists() {
            "/opt/homebrew/bin/fish".to_string()
        } else if std::path::Path::new("/usr/bin/fish").exists() {
            "/usr/bin/fish".to_string()
        } else if std::path::Path::new("/usr/local/bin/fish").exists() {
            "/usr/local/bin/fish".to_string()
        } else if std::path::Path::new("/system/bin/sh").exists() {
            "/system/bin/sh".to_string()
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
        };

        let mut cmd = CommandBuilder::new(&shell);
        if cfg!(unix) {
            cmd.arg("-l");
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "Well-Shell");
        cmd.env("WELL_SHELL", "1");
        cmd.env("fish_greeting", "Well-Shell");

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

        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 10_000)));
        let parser_clone = Arc::clone(&parser);

        let (gfx_tx, gfx_rx) = channel();

        // Optional history file logging
        let mut log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/well-history.log")
            .ok();

        // Background reader thread: captures raw ANSI/VT escape stream, responds to terminal queries,
        // strips query sequences from the output stream, and processes into vt100 screen buffer
        thread::Builder::new()
            .name("well-pty-reader".to_string())
            .spawn(move || {
                let mut buf = [0u8; 65536];
                let mut interceptor = Interceptor::new();
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            let slice = &buf[..n];

                            if let Some(f) = log_file.as_mut() {
                                let _ = f.write_all(slice);
                            }

                            // 1. Intercept graphic protocols (APC Kitty Images, OSC 7/8)
                            let sanitized_gfx = interceptor.process(slice);

                            // Send intercepted events to consumer
                            while let Some(event) = interceptor.events.pop_front() {
                                let _ = gfx_tx.send(event);
                            }

                            // 2. Filter queries and respond into child PTY stdin
                            let sanitized = if sanitized_gfx.contains(&0x1b) {
                                if let Ok(mut w) = writer_clone.lock() {
                                    filter_and_respond_queries(&sanitized_gfx, &parser_clone, &mut **w)
                                } else {
                                    sanitized_gfx
                                }
                            } else {
                                sanitized_gfx
                            };

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
            graphic_events_rx: Mutex::new(gfx_rx),
        })
    }

    /// Sends raw keystrokes or escape sequences to child process stdin
    pub fn write_all(&self, bytes: &[u8]) -> std::io::Result<()> {
        let mut w = self
            .writer
            .lock()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        w.write_all(bytes)?;
        w.flush()?;
        Ok(())
    }

    /// Resizes both the kernel PTY driver and the vt100 virtual grid
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    /// Check if child process is still running
    pub fn is_alive(&self) -> bool {
        if let Ok(mut child) = self.child.lock() {
            child
                .try_wait()
                .map(|status| status.is_none())
                .unwrap_or(false)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_session_spawns_and_reads() {
        let session = PtySession::spawn(24, 80, || {}).expect("Failed to spawn PTY");
        std::thread::sleep(std::time::Duration::from_millis(600));
        let parser = session.parser.lock().unwrap();
        let contents = parser.screen().contents();
        assert!(session.is_alive());
        println!("PTY SCREEN CONTENTS:\n{:?}", contents);
    }

    #[test]
    fn test_filter_and_respond_queries_da1() {
        let parser = Arc::new(Mutex::new(vt100::Parser::new(24, 80, 1000)));
        let mut response = Vec::new();
        let sanitized = filter_and_respond_queries(b"prefix\x1b[csuffix", &parser, &mut response);
        assert_eq!(response, b"\x1b[?62;1;2;6;7;8;9c");
        assert_eq!(sanitized, b"prefixsuffix");
    }

    #[test]
    fn test_filter_and_respond_queries_da2() {
        let parser = Arc::new(Mutex::new(vt100::Parser::new(24, 80, 1000)));
        let mut response = Vec::new();
        let sanitized = filter_and_respond_queries(b"\x1b[>c", &parser, &mut response);
        assert_eq!(response, b"\x1b[>0;10;1c");
        assert!(sanitized.is_empty());
    }

    #[test]
    fn test_filter_and_respond_queries_kitty_and_osc() {
        let parser = Arc::new(Mutex::new(vt100::Parser::new(24, 80, 1000)));
        let mut response = Vec::new();
        let sanitized = filter_and_respond_queries(b"\x1b[?u\x1b]11;?\x07data", &parser, &mut response);
        assert_eq!(sanitized, b"data");
        assert!(response.windows(4).any(|w| w == b"\x1b[?0"));
        assert!(response.windows(5).any(|w| w == b"\x1b]11;"));
    }
}
