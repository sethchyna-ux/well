//! pty.rs: Pseudo-Terminal (PTY) Subsystem for Well Terminal
//!
//! Provides bidirectional streaming between the OS shell (zsh/bash) and
//! the Orpheus/Atlas rendering pipeline via portable-pty and vt100.

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct PtySession {
    pub parser: Arc<Mutex<vt100::Parser>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

impl PtySession {
    /// Spawns a new interactive login shell (defaulting to /opt/homebrew/bin/fish, $SHELL, or /bin/zsh)
    /// connected to a virtual terminal screen buffer of dimensions (rows, cols).
    pub fn spawn<F>(rows: u16, cols: u16, on_output: F) -> Result<Self, Box<dyn std::error::Error>>
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
        on_output: F,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: Fn() + Send + 'static,
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
        } else if std::path::Path::new("/opt/homebrew/bin/fish").exists() {
            "/opt/homebrew/bin/fish".to_string()
        } else if std::path::Path::new("/usr/local/bin/fish").exists() {
            "/usr/local/bin/fish".to_string()
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string())
        };
        let mut cmd = CommandBuilder::new(&shell);
        cmd.arg("-l");
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        if let Ok(home) = std::env::var("HOME") {
            cmd.cwd(home);
        }

        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave); // Vital: closes slave fd in parent so reader EOF functions correctly on shell exit
        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;

        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 10_000)));
        let parser_clone = Arc::clone(&parser);

        // Background reader thread: captures raw ANSI/VT escape stream and processes into vt100 screen
        thread::Builder::new()
            .name("well-pty-reader".to_string())
            .spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            if let Ok(mut p) = parser_clone.lock() {
                                p.process(&buf[..n]);
                            }
                            on_output();
                        }
                        Err(_) => break,
                    }
                }
            })?;

        Ok(Self {
            parser,
            writer: Arc::new(Mutex::new(writer)),
            master: Arc::new(Mutex::new(pair.master)),
            child: Arc::new(Mutex::new(child)),
        })
    }

    /// Sends raw keystrokes or escape sequences to child process stdin
    pub fn write_all(&self, bytes: &[u8]) -> std::io::Result<()> {
        let mut w = self.writer.lock().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
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

    /// Check if child process is still running
    pub fn is_alive(&self) -> bool {
        if let Ok(mut child) = self.child.lock() {
            child.try_wait().map(|status| status.is_none()).unwrap_or(false)
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
        // Give shell 600ms to produce initial prompt or login banner
        std::thread::sleep(std::time::Duration::from_millis(600));
        let parser = session.parser.lock().unwrap();
        let contents = parser.screen().contents();
        println!("PTY SCREEN CONTENTS:\n{:?}", contents);
        println!("CURSOR POS: {:?}", parser.screen().cursor_position());
    }
}
