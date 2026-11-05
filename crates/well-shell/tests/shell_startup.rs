#![cfg(unix)]

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use well_shell::pty::PtySession;

const WAIT_TIMEOUT: Duration = Duration::from_secs(8);

struct ShellHarness {
    session: PtySession,
    output: Arc<Mutex<Vec<u8>>>,
}

impl ShellHarness {
    fn spawn(shell: &str) -> Self {
        let output = Arc::new(Mutex::new(Vec::new()));
        let captured_output = Arc::clone(&output);
        let session = PtySession::spawn_with_stream(24, 80, Some(shell), move |chunk| {
            captured_output.lock().unwrap().extend_from_slice(chunk);
        })
        .unwrap_or_else(|error| panic!("failed to spawn shell {shell:?}: {error}"));

        Self { session, output }
    }

    fn send(&self, input: &[u8]) {
        self.session
            .write_all(input)
            .expect("failed to write to shell PTY");
    }

    fn output(&self) -> String {
        String::from_utf8_lossy(&self.output.lock().unwrap()).into_owned()
    }

    fn wait_for(&self, description: &str, expected: &str) {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        while Instant::now() < deadline {
            if self.output().contains(expected) {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "timed out waiting for {description}; expected={expected:?}; output={:?}",
            self.output()
                .chars()
                .rev()
                .take(1_200)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        );
    }
}

fn shell_candidates() -> Vec<String> {
    let mut shells = BTreeSet::new();
    shells.insert("/bin/sh".to_string());

    for candidate in [
        std::env::var("SHELL").ok(),
        Some("/bin/zsh".to_string()),
        Some("/opt/homebrew/bin/fish".to_string()),
        Some("/usr/local/bin/fish".to_string()),
        Some("/usr/bin/fish".to_string()),
    ]
    .into_iter()
    .flatten()
    {
        if Path::new(&candidate).is_file() {
            shells.insert(candidate);
        }
    }

    shells.into_iter().collect()
}

#[test]
#[ignore = "requires local shells; run scripts/smoke-shell-startup.sh"]
fn available_login_shells_start_run_resize_and_exit() {
    let shells = shell_candidates();
    assert!(!shells.is_empty());

    for shell in shells {
        let marker = format!("__well_shell_startup_{}__", shell.replace('/', "_"));
        let harness = ShellHarness::spawn(&shell);

        harness.send(format!("printf '{marker}\\n'\n").as_bytes());
        harness.wait_for("startup marker", &marker);

        harness
            .session
            .resize(33, 101)
            .expect("failed to resize shell PTY");
        harness.send(b"stty size\n");
        harness.wait_for("resized PTY size", "33 101");

        harness.send(b"exit 0\n");
        let status = harness
            .session
            .wait_for_exit(WAIT_TIMEOUT)
            .expect("failed to wait for shell exit")
            .unwrap_or_else(|| panic!("shell {shell:?} did not exit"));
        assert!(status.success(), "shell {shell:?} exited with {status:?}");
    }
}
