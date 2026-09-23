#![cfg(unix)]

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use well_shell::pty::PtySession;

const WAIT_TIMEOUT: Duration = Duration::from_secs(8);

struct PtyHarness {
    session: PtySession,
    output: Arc<Mutex<Vec<u8>>>,
}

impl PtyHarness {
    fn spawn() -> Self {
        let output = Arc::new(Mutex::new(Vec::new()));
        let captured_output = Arc::clone(&output);
        let session = PtySession::spawn_with_stream(30, 100, Some("/bin/sh"), move |chunk| {
            captured_output.lock().unwrap().extend_from_slice(chunk);
        })
        .expect("failed to spawn Well PTY compatibility shell");

        let harness = Self { session, output };
        harness.send(b"stty -echo\n");
        thread::sleep(Duration::from_millis(100));
        harness.output.lock().unwrap().clear();
        harness.shell_marker("ready");
        harness
    }

    fn send(&self, input: &[u8]) {
        self.session
            .write_all(input)
            .expect("failed to write to compatibility PTY");
    }

    fn raw_output(&self) -> String {
        String::from_utf8_lossy(&self.output.lock().unwrap()).into_owned()
    }

    fn screen_contents(&self) -> String {
        self.session.parser.lock().unwrap().screen().contents()
    }

    fn wait_for(&self, description: &str, predicate: impl Fn() -> bool) {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        while Instant::now() < deadline {
            if predicate() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }

        panic!(
            "timed out waiting for {description}; screen={:?}; recent output={:?}",
            self.screen_contents(),
            self.raw_output()
                .chars()
                .rev()
                .take(1_000)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        );
    }

    fn wait_for_raw_text(&self, text: &str) {
        self.wait_for(&format!("terminal output containing {text:?}"), || {
            self.raw_output().contains(text)
        });
    }

    fn wait_for_alternate_screen(&self, expected: bool) {
        self.wait_for(
            if expected {
                "alternate-screen entry"
            } else {
                "alternate-screen exit"
            },
            || {
                self.session
                    .parser
                    .lock()
                    .unwrap()
                    .screen()
                    .alternate_screen()
                    == expected
            },
        );
    }

    fn shell_marker(&self, name: &str) {
        let marker = format!("__well_{name}_complete__");
        self.send(format!("printf '{marker}\\n'\n").as_bytes());
        self.wait_for_raw_text(&marker);
    }
}

fn require_program(name: &str) {
    let status = Command::new("/bin/sh")
        .args(["-c", &format!("command -v {name} >/dev/null 2>&1")])
        .status()
        .expect("failed to inspect installed programs");
    assert!(
        status.success(),
        "required compatibility program {name:?} is not installed"
    );
}

#[test]
#[ignore = "requires local interactive programs; run scripts/smoke-interactive-programs.sh"]
fn common_interactive_programs_work_through_well_pty() {
    for program in ["vim", "less", "ssh", "tmux", "top"] {
        require_program(program);
    }

    let harness = PtyHarness::spawn();

    harness.send(b"vim -Nu NONE -n -i NONE\n");
    harness.wait_for_alternate_screen(true);
    harness.send(b"\x1b:qa!\r");
    harness.wait_for_alternate_screen(false);
    harness.shell_marker("vim");

    harness.send(b"printf 'well less smoke\\n' | less\n");
    harness.wait_for_alternate_screen(true);
    harness.send(b"q");
    harness.wait_for_alternate_screen(false);
    harness.shell_marker("less");

    harness.send(b"ssh -V\n");
    harness.wait_for_raw_text("OpenSSH_");
    harness.shell_marker("ssh");

    let tmux_socket = format!("well-smoke-{}", std::process::id());
    harness.send(format!("tmux -L {tmux_socket} new-session -s well-smoke\n").as_bytes());
    harness.wait_for_alternate_screen(true);
    harness.send(b"exit\r");
    harness.wait_for_alternate_screen(false);
    harness.shell_marker("tmux");

    harness.send(b"top\n");
    harness.wait_for("top process display", || {
        let output = harness.raw_output();
        output.contains("Processes:") || output.contains("load averages:")
    });
    harness.send(b"q");
    harness.shell_marker("top");

    harness.send(b"exit 0\n");
    let status = harness
        .session
        .wait_for_exit(WAIT_TIMEOUT)
        .expect("failed to wait for compatibility shell")
        .expect("compatibility shell did not exit");
    assert!(status.success());
}
