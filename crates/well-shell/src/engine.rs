//! engine.rs: ExecutionRouter & Dual-Path Command Engine
//!
//! Subsystems:
//! - ExecutionRouter: Intelligently routes commands between interactive PTY shims (for vim, htop, ssh)
//!   and bounded IPC streaming pipelines (for high-throughput batch commands).

use crate::keyboard::KeyEncoder;
use crate::pty::PtySession;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use well_ipc::ring_buffer::Producer;
use well_ipc::{KittyKeyboardInput, TypedBlock};

pub struct ExecutionRouter<const N: usize> {
    pub producer: Arc<Producer<TypedBlock, N>>,
}

impl<const N: usize> ExecutionRouter<N> {
    pub fn new(producer: Producer<TypedBlock, N>) -> Self {
        Self {
            producer: Arc::new(producer),
        }
    }

    /// Handles incoming Kitty Keyboard Input events.
    pub fn handle_input(
        &self,
        input: KittyKeyboardInput,
        pty: Option<&mut PtySession>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(pty_session) = pty {
            if let Some(legacy_bytes) = KeyEncoder::encode_legacy(&input) {
                pty_session.write_all(&legacy_bytes)?;
            }
        }
        Ok(())
    }

    /// Determines if a command is typically an interactive TUI program requiring a PTY.
    pub fn is_interactive(cmd: &str) -> bool {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return false;
        }

        let program = parts[0];
        matches!(
            program,
            "htop"
                | "top"
                | "less"
                | "more"
                | "ssh"
                | "vim"
                | "vi"
                | "nvim"
                | "nano"
                | "tmux"
                | "screen"
        )
    }

    /// Executes the given command using the dual-path execution strategy.
    /// Returns true if it was run in the legacy PTY shim, false if it used the Native IPC path.
    pub async fn execute(
        &self,
        cmd: &str,
        rows: u16,
        cols: u16,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if Self::is_interactive(cmd) {
            // Legacy Compatibility Shim (PTY)
            let session = PtySession::spawn_with_stream(rows, cols, None, |_| {})?;
            session.write_all(format!("{}\n", cmd).as_bytes())?;
            Ok(true)
        } else {
            // Native Path (Hermes IPC Ring Buffer)
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if parts.is_empty() {
                return Ok(false);
            }

            let mut child = Command::new("/bin/sh")
                .arg("-c")
                .arg(cmd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            let mut stdout = child
                .stdout
                .take()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "stdout failed"))?;
            let mut stderr = child
                .stderr
                .take()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "stderr failed"))?;

            let producer_out = Arc::clone(&self.producer);
            let producer_err = Arc::clone(&self.producer);

            let stdout_task = tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                loop {
                    match stdout.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let block = TypedBlock::StdoutChunk(buf[..n].to_vec());
                            producer_out
                                .push(block)
                                .await
                                .expect("shell output queue must have capacity");
                        }
                        Err(_) => break,
                    }
                }
            });

            let stderr_task = tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                loop {
                    match stderr.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let block = TypedBlock::StderrChunk(buf[..n].to_vec());
                            producer_err
                                .push(block)
                                .await
                                .expect("shell output queue must have capacity");
                        }
                        Err(_) => break,
                    }
                }
            });

            let producer_exit = Arc::clone(&self.producer);
            tokio::spawn(async move {
                let status = child.wait().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                let exit_code = match status {
                    Ok(s) => s.code().unwrap_or(-1),
                    Err(_) => -1,
                };
                producer_exit
                    .push(TypedBlock::ExitStatus(exit_code))
                    .await
                    .expect("shell output queue must have capacity");
            });

            Ok(false)
        }
    }

    /// Executes the given command using sandbox isolation
    pub async fn execute_sandboxed(
        &self,
        cmd: &str,
        args: &[String],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut command = if cfg!(target_os = "macos") {
            let mut c = Command::new("sandbox-exec");
            c.arg("-n").arg("no-network").arg(cmd);
            c
        } else if cfg!(target_os = "linux") {
            let mut c = Command::new("unshare");
            c.arg("--net").arg("--mount").arg(cmd);
            c
        } else {
            Command::new(cmd)
        };

        command.args(args);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn()?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "stdout failed"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "stderr failed"))?;

        let producer_out = Arc::clone(&self.producer);
        let producer_err = Arc::clone(&self.producer);

        let stdout_task = tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let block = TypedBlock::StdoutChunk(buf[..n].to_vec());
                        producer_out
                            .push(block)
                            .await
                            .expect("shell output queue must have capacity");
                    }
                    Err(_) => break,
                }
            }
        });

        let stderr_task = tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match stderr.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let block = TypedBlock::StderrChunk(buf[..n].to_vec());
                        producer_err
                            .push(block)
                            .await
                            .expect("shell output queue must have capacity");
                    }
                    Err(_) => break,
                }
            }
        });

        let producer_exit = Arc::clone(&self.producer);
        tokio::spawn(async move {
            let status = child.wait().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            let exit_code = match status {
                Ok(s) => s.code().unwrap_or(-1),
                Err(_) => -1,
            };
            producer_exit
                .push(TypedBlock::ExitStatus(exit_code))
                .await
                .expect("shell output queue must have capacity");
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use well_ipc::ring_buffer::RingBuffer;

    #[tokio::test]
    async fn test_native_path_typed_blocks() {
        let ring_buffer = RingBuffer::<TypedBlock, 128>::new();
        let (producer, mut consumer) = ring_buffer.split();
        let router = ExecutionRouter::new(producer);

        let is_legacy = router.execute("echo hello", 24, 80).await.unwrap();
        assert!(!is_legacy);

        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut got_stdout = false;
        let mut got_exit = false;

        while let Some(block) = consumer.try_pop() {
            match block {
                TypedBlock::StdoutChunk(data) => {
                    let text = String::from_utf8_lossy(&data);
                    if text.contains("hello") {
                        got_stdout = true;
                    }
                }
                TypedBlock::ExitStatus(code) => {
                    assert_eq!(code, 0);
                    got_exit = true;
                }
                _ => {}
            }
        }

        assert!(got_stdout, "Failed to receive typed stdout block");
        assert!(got_exit, "Failed to receive typed exit status block");
    }

    #[tokio::test]
    async fn native_path_preserves_shell_syntax_and_sends_exit_after_output() {
        let ring_buffer = RingBuffer::<TypedBlock, 8>::new();
        let (producer, mut consumer) = ring_buffer.split();
        let router = ExecutionRouter::new(producer);

        assert!(!router
            .execute("printf 'hello world'; exit 7", 24, 80)
            .await
            .unwrap());

        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        let mut stdout = Vec::new();
        let mut exit_code = None;
        while tokio::time::Instant::now() < deadline {
            match consumer.try_pop() {
                Some(TypedBlock::StdoutChunk(data)) => stdout.extend(data),
                Some(TypedBlock::ExitStatus(code)) => {
                    exit_code = Some(code);
                    break;
                }
                Some(TypedBlock::StderrChunk(_)) => {}
                None => tokio::task::yield_now().await,
            }
        }

        assert_eq!(stdout, b"hello world");
        assert_eq!(exit_code, Some(7));
    }

    #[test]
    fn test_is_interactive() {
        assert!(ExecutionRouter::<128>::is_interactive("htop"));
        assert!(ExecutionRouter::<128>::is_interactive("ssh user@host"));
        assert!(!ExecutionRouter::<128>::is_interactive("ls -la"));
        assert!(!ExecutionRouter::<128>::is_interactive("cargo build"));
    }
}
