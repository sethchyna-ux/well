use std::io::{self, Write};
use std::process::{Command, Output, Stdio};

/// Isolated re-forking environment for historical replay.
pub struct Sandbox {
    cmd: Command,
}

impl Sandbox {
    pub fn new(program: &str) -> Self {
        let mut cmd = Command::new(program);

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        Self { cmd }
    }

    /// Spawns the sandbox process, pipes historical stdin bytes into it, and collects output.
    pub fn run_block(&mut self, stdin_bytes: &[u8]) -> io::Result<Output> {
        let mut child = self.cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(stdin_bytes);
            let _ = stdin.flush();
        }
        child.wait_with_output()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_stdin_pipe_and_output() {
        let mut sandbox = Sandbox::new("cat");
        let output = sandbox
            .run_block(b"hello historical sandbox\n")
            .expect("run_block");
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "hello historical sandbox\n"
        );
    }
}
