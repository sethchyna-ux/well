use crate::pty::PtySession;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use well_ipc::ring_buffer::Producer;
use well_ipc::TypedBlock;

use crate::keyboard::KeyEncoder;
use well_ipc::KittyKeyboardInput;

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
    /// In a fully stateful router, this would route to the active PTY's `write` handle
    /// or directly process the AST command.
    pub fn handle_input(
        &self,
        input: KittyKeyboardInput,
        pty: Option<&mut PtySession>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(pty_session) = pty {
            if let Some(legacy_bytes) = KeyEncoder::encode_legacy(&input) {
                pty_session.write_all(&legacy_bytes)?;
            }
        } else {
            // Native path AST handling
            // Here we would append to the current prompt buffer
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

    fn push_to_producer(producer: &Producer<TypedBlock, N>, block: TypedBlock) {
        while producer.try_push(block.clone()).is_err() {
            std::hint::spin_loop();
        }
    }

    /// Executes the given command using the dual-path execution strategy.
    /// Returns true if it was run in the legacy PTY shim, false if it used the Native IPC path.
    pub async fn execute(
        &self,
        cmd: &str,
        rows: u16,
        cols: u16,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if Self::is_interactive(cmd) {
            // Legacy Compatibility Shim (PTY)
            let _session = PtySession::spawn_with_stream(rows, cols, None, |_| {
                // In a full implementation, we'd pipe this to some screen buffer
                // For this demo, we just sink it.
            })?;

            _session.write_all(format!("{}\n", cmd).as_bytes())?;

            Ok(true)
        } else {
            // Native Path (Hermes IPC)
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if parts.is_empty() {
                return Ok(false);
            }

            let mut child = Command::new(parts[0])
                .args(&parts[1..])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            let mut stdout = child.stdout.take().ok_or("child stdout was not piped")?;
            let mut stderr = child.stderr.take().ok_or("child stderr was not piped")?;

            let producer_out = Arc::clone(&self.producer);
            let producer_err = Arc::clone(&self.producer);

            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                loop {
                    match stdout.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let block = TypedBlock::StdoutChunk(buf[..n].to_vec());
                            Self::push_to_producer(&producer_out, block);
                        }
                        Err(_) => break,
                    }
                }
            });

            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                loop {
                    match stderr.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let block = TypedBlock::StderrChunk(buf[..n].to_vec());
                            Self::push_to_producer(&producer_err, block);
                        }
                        Err(_) => break,
                    }
                }
            });

            let producer_exit = Arc::clone(&self.producer);
            tokio::spawn(async move {
                let block = match child.wait().await {
                    Ok(status) => TypedBlock::ExitStatus(status.code().unwrap_or(-1)),
                    Err(err) => TypedBlock::StderrChunk(
                        format!("failed to wait for child process: {err}\n").into_bytes(),
                    ),
                };
                Self::push_to_producer(&producer_exit, block);
            });

            Ok(false)
        }
    }

    /// Executes the given command using the Sandbox environment
    pub async fn execute_sandboxed(
        &self,
        cmd: &str,
        args: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
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

        let mut stdout = child.stdout.take().ok_or("child stdout was not piped")?;
        let mut stderr = child.stderr.take().ok_or("child stderr was not piped")?;

        let producer_out = Arc::clone(&self.producer);
        let producer_err = Arc::clone(&self.producer);

        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let block = TypedBlock::StdoutChunk(buf[..n].to_vec());
                        Self::push_to_producer(&producer_out, block);
                    }
                    Err(_) => break,
                }
            }
        });

        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match stderr.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let block = TypedBlock::StderrChunk(buf[..n].to_vec());
                        Self::push_to_producer(&producer_err, block);
                    }
                    Err(_) => break,
                }
            }
        });

        let producer_exit = Arc::clone(&self.producer);
        tokio::spawn(async move {
            let block = match child.wait().await {
                Ok(status) => TypedBlock::ExitStatus(status.code().unwrap_or(-1)),
                Err(err) => TypedBlock::StderrChunk(
                    format!("failed to wait for sandboxed child process: {err}\n").into_bytes(),
                ),
            };
            Self::push_to_producer(&producer_exit, block);
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

        // Execute a non-interactive command
        let is_legacy = router.execute("echo hello", 24, 80).await.unwrap();
        assert!(!is_legacy);

        // Wait briefly for execution
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

    #[test]
    fn test_is_interactive() {
        assert!(ExecutionRouter::<128>::is_interactive("htop"));
        assert!(ExecutionRouter::<128>::is_interactive("ssh user@host"));
        assert!(!ExecutionRouter::<128>::is_interactive("ls -la"));
        assert!(!ExecutionRouter::<128>::is_interactive("cargo build"));
    }
}
