//! Adversarial stress test harness for `well-core`.
//! Tests RingBuffer memory safety, high-volume wraparound, and concurrent access.
//! Tests Pythia shell safety (`detect_destructive_command`) against bypass vectors,
//! Unicode escapes, complex pipelines, wrappers, and pathological inputs.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Instant;

use well_core::ai::detect_destructive_command;
use well_core::{RingBuffer, SessionCoordinator, SurfaceId};

// =========================================================================
// 1. RingBuffer Adversarial Stress & Memory Safety Tests
// =========================================================================

#[test]
#[should_panic(expected = "greater than zero")]
fn test_ring_buffer_zero_capacity_panics() {
    let _ = RingBuffer::<u32>::new(0);
}

#[test]
fn test_ring_buffer_single_element_capacity() {
    let mut rb = RingBuffer::<String>::new(1);
    assert_eq!(rb.capacity(), 1);
    assert_eq!(rb.len(), 0);
    assert!(rb.is_empty());
    assert_eq!(rb.latest(), None);

    rb.push("alpha".to_string());
    assert_eq!(rb.len(), 1);
    assert!(!rb.is_empty());
    assert_eq!(rb.latest().map(String::as_str), Some("alpha"));

    rb.push("beta".to_string());
    assert_eq!(rb.len(), 1);
    assert_eq!(rb.latest().map(String::as_str), Some("beta"));

    rb.clear();
    assert_eq!(rb.len(), 0);
    assert!(rb.is_empty());
    assert_eq!(rb.latest(), None);
}

/// Tracks drop counts to empirically prove zero memory leaks under overwrites and clear().
struct DropTracker {
    _data: Vec<u8>,
    drop_counter: Arc<AtomicUsize>,
}

impl DropTracker {
    fn new(drop_counter: Arc<AtomicUsize>) -> Self {
        Self {
            _data: vec![0xAA; 1024], // 1 KB heap allocation per item
            drop_counter,
        }
    }
}

impl Drop for DropTracker {
    fn drop(&mut self) {
        self.drop_counter.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn test_ring_buffer_drop_semantics_and_zero_memory_leak() {
    let drops = Arc::new(AtomicUsize::new(0));
    const CAPACITY: usize = 50;
    const TOTAL_PUSHES: usize = 10_000;

    {
        let mut rb = RingBuffer::<DropTracker>::new(CAPACITY);
        for _ in 0..TOTAL_PUSHES {
            rb.push(DropTracker::new(drops.clone()));
        }
        assert_eq!(rb.len(), CAPACITY);
        // Overwritten items must have been dropped immediately on overwrite:
        // TOTAL_PUSHES - CAPACITY items dropped so far
        assert_eq!(
            drops.load(Ordering::SeqCst),
            TOTAL_PUSHES - CAPACITY,
            "Overwritten items must be dropped immediately on push"
        );
    }
    // When the RingBuffer is dropped, remaining items in the buffer must be dropped.
    assert_eq!(
        drops.load(Ordering::SeqCst),
        TOTAL_PUSHES,
        "All pushed items must be dropped with exactly 0 leaks"
    );
}

#[test]
fn test_ring_buffer_clear_drops_elements_and_resets() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut rb = RingBuffer::<DropTracker>::new(20);

    for _ in 0..15 {
        rb.push(DropTracker::new(drops.clone()));
    }
    assert_eq!(rb.len(), 15);
    assert_eq!(drops.load(Ordering::SeqCst), 0);

    rb.clear();
    assert_eq!(rb.len(), 0);
    assert!(rb.is_empty());
    assert_eq!(rb.latest().is_none(), true);
    assert_eq!(
        drops.load(Ordering::SeqCst),
        15,
        "clear() must drop all currently stored items"
    );

    // Push new items after clear()
    for _ in 0..5 {
        rb.push(DropTracker::new(drops.clone()));
    }
    assert_eq!(rb.len(), 5);
}

#[test]
fn test_ring_buffer_high_volume_wraparound_stress() {
    const CAPACITY: usize = 128;
    const ITERATIONS: usize = 1_000_000;

    let mut rb = RingBuffer::<usize>::new(CAPACITY);
    let start = Instant::now();

    for i in 0..ITERATIONS {
        rb.push(i);
    }

    let elapsed = start.elapsed();
    assert_eq!(rb.len(), CAPACITY);
    assert_eq!(rb.latest(), Some(&(ITERATIONS - 1)));
    assert!(
        elapsed.as_millis() < 500,
        "1M ring buffer pushes took {:?}, exceeding 500ms budget",
        elapsed
    );
}

#[test]
fn test_ring_buffer_concurrent_multithreaded_read_write() {
    const CAPACITY: usize = 256;
    const WRITERS: usize = 4;
    const ITEMS_PER_WRITER: usize = 25_000;

    let rb = Arc::new(RwLock::new(RingBuffer::<usize>::new(CAPACITY)));
    let mut handles = Vec::new();

    // Spawn concurrent writers
    for writer_id in 0..WRITERS {
        let rb_clone = Arc::clone(&rb);
        handles.push(thread::spawn(move || {
            for i in 0..ITEMS_PER_WRITER {
                let val = writer_id * ITEMS_PER_WRITER + i;
                let mut guard = rb_clone.write().unwrap();
                guard.push(val);
            }
        }));
    }

    // Spawn concurrent readers
    for _ in 0..4 {
        let rb_clone = Arc::clone(&rb);
        handles.push(thread::spawn(move || {
            for _ in 0..10_000 {
                let guard = rb_clone.read().unwrap();
                let _ = guard.latest();
                let _ = guard.len();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let guard = rb.read().unwrap();
    assert_eq!(guard.len(), CAPACITY);
    assert!(guard.latest().is_some());
}

// =========================================================================
// 2. SessionCoordinator Stress Tests
// =========================================================================

#[test]
fn test_session_coordinator_rapid_churn() {
    let mut coord = SessionCoordinator::new();
    const SESSIONS: usize = 10_000;
    let mut ids = Vec::with_capacity(SESSIONS);

    for i in 0..SESSIONS {
        let id = coord.create_session(format!("Session {}", i), "/bin/zsh", 24, 80);
        ids.push(id);
    }
    assert_eq!(coord.session_count(), SESSIONS);

    // Close half from middle
    for id in ids.drain(2500..7500) {
        let removed = coord.close_session(id);
        assert!(removed.is_some());
    }
    assert_eq!(coord.session_count(), 5000);

    // Focus non-existent surface returns false
    assert!(!coord.focus_surface(SurfaceId(999_999_999)));
}

// =========================================================================
// 3. Pythia detect_destructive_command Adversarial Verification
// =========================================================================

#[test]
fn test_detects_standard_dangerous_commands() {
    assert!(detect_destructive_command("rm -rf /"));
    assert!(detect_destructive_command("rmdir /tmp/dir"));
    assert!(detect_destructive_command("unlink obsolete.sock"));
    assert!(detect_destructive_command("shred -u secret.key"));
    assert!(detect_destructive_command("truncate -s 0 /var/log/syslog"));
    assert!(detect_destructive_command("dd if=/dev/urandom of=/dev/nvme0n1"));
    assert!(detect_destructive_command("mkfs.vfat /dev/sdb1"));
    assert!(detect_destructive_command("diskutil eraseVolume APFS Blank /dev/disk3"));
    assert!(detect_destructive_command("chmod -R 000 /"));
    assert!(detect_destructive_command("chown root:wheel /etc/shadow"));
}

#[test]
fn test_detects_nested_wrappers_and_execution_chains() {
    // Nested wrappers: sudo -> env -> time -> nohup -> nice -> timeout -> rm
    assert!(detect_destructive_command(
        "sudo env LC_ALL=C /usr/bin/time -p nohup nice -n 10 timeout 30 rm -rf /tmp/target"
    ));

    // doas with user specification
    assert!(detect_destructive_command("doas -u operator rm -f /var/run/app.pid"));

    // env with multiple variable assignments and flags
    assert!(detect_destructive_command("env -i PATH=/bin USER=root LC_ALL=C rm test.txt"));
}

#[test]
fn test_detects_compound_shell_separators_and_substitutions() {
    // Semicolon, newlines, pipes, background operators
    assert!(detect_destructive_command("echo starting; rm -rf target"));
    assert!(detect_destructive_command("echo starting\nrm -rf target"));
    assert!(detect_destructive_command("echo yes | rm -i file.txt"));
    assert!(detect_destructive_command("echo background & rm -rf target &"));
    assert!(detect_destructive_command("git status && rm -rf target"));
    assert!(detect_destructive_command("git status || rm -rf target"));

    // Deep command substitutions
    assert!(detect_destructive_command("echo $(echo $(echo $(rm -rf /)))"));
    assert!(detect_destructive_command("VAR=\"`rm -rf /`\""));
}

#[test]
fn test_permits_safe_commands_and_quoted_mentions() {
    assert!(!detect_destructive_command("echo 'rm -rf /'"));
    assert!(!detect_destructive_command("cat \"$(echo 'safe text')\""));
    assert!(!detect_destructive_command("echo '\\$(rm -rf /)'"));
    assert!(!detect_destructive_command("git status"));
    assert!(!detect_destructive_command("cargo build --release"));
    assert!(!detect_destructive_command("find . -name '*.rs' -print"));
    assert!(!detect_destructive_command("echo ':(){ :|:& };:'"));
}

#[test]
fn test_pathological_inputs_do_not_panic_or_hang() {
    // Deeply nested parentheses (1000 levels)
    let mut deep_subst = String::new();
    for _ in 0..1000 {
        deep_subst.push_str("$(");
    }
    deep_subst.push_str("ls");
    for _ in 0..1000 {
        deep_subst.push(')');
    }
    let start = Instant::now();
    let _ = detect_destructive_command(&deep_subst);
    assert!(start.elapsed().as_millis() < 500, "Deeply nested substitutions must finish promptly");

    // Huge input string (1 MB of spaces and commands)
    let huge_input = "echo safe ; ".repeat(50_000);
    let start = Instant::now();
    let _ = detect_destructive_command(&huge_input);
    assert!(start.elapsed().as_millis() < 1000, "1MB command string evaluated within budget");

    // Repeated backslashes
    let backslashes = "\\".repeat(10_000);
    let _ = detect_destructive_command(&backslashes);

    // Unclosed single and double quotes
    assert!(!detect_destructive_command("'unclosed single quote"));
    assert!(!detect_destructive_command("\"unclosed double quote"));
}

// =========================================================================
// 4. Adversarial Findings & Documented Bypasses for Pythia Safety Gate
// =========================================================================

#[test]
fn test_adversarial_findings_documented() {
    // The following tests document edge cases and limitations of the heuristic lexer
    // as acknowledged in the design comments ("conservative warning heuristic, not a sandbox").

    // Finding 1: ANSI-C quoting with escape sequences ($'\x72\x6d' -> "rm")
    // The lexer produces the literal token "$'\\x72\\x6d'" which does not match "rm".
    let ansi_c_hex = "$'\\x72\\x6d' -rf /";
    let is_detected_ansi_hex = detect_destructive_command(ansi_c_hex);
    // Documenting empirical reality: currently false due to lack of ANSI-C quote expansion
    assert_eq!(
        is_detected_ansi_hex, false,
        "Documenting heuristic boundary: ANSI-C hex escape ($'\\x72\\x6d') bypasses naive lexer"
    );

    // Finding 2: Unhandled wrappers: pkexec, su, busybox, toybox, chroot
    let pkexec_rm = "pkexec rm -rf /";
    assert_eq!(
        detect_destructive_command(pkexec_rm),
        false,
        "Documenting heuristic boundary: pkexec is not in wrapper table"
    );

    let su_rm = "su root -c 'rm -rf /'";
    assert_eq!(
        detect_destructive_command(su_rm),
        false,
        "Documenting heuristic boundary: su is not in wrapper table"
    );

    let busybox_rm = "busybox rm -rf /";
    assert_eq!(
        detect_destructive_command(busybox_rm),
        false,
        "Documenting heuristic boundary: busybox is not in wrapper table"
    );

    // Finding 3: Subshell and block wrapping
    let subshell_rm = "(rm -rf /)";
    assert_eq!(
        detect_destructive_command(subshell_rm),
        false,
        "Documenting heuristic boundary: (rm -rf /) produces token '(rm' which does not match 'rm'"
    );

    let block_rm = "{ rm -rf /; }";
    assert_eq!(
        detect_destructive_command(block_rm),
        false,
        "Documenting heuristic boundary: '{{' is treated as command token"
    );

    // Finding 4: kubectl and terraform with flags before subcommands
    let kubectl_with_namespace = "kubectl -n production delete pod my-pod";
    assert_eq!(
        detect_destructive_command(kubectl_with_namespace),
        false,
        "Documenting heuristic boundary: kubectl with -n flag has first arg '-n' instead of 'delete'"
    );

    let terraform_with_chdir = "terraform -chdir=prod destroy -auto-approve";
    assert_eq!(
        detect_destructive_command(terraform_with_chdir),
        false,
        "Documenting heuristic boundary: terraform with -chdir flag has first arg '-chdir=prod' instead of 'destroy'"
    );

    // Finding 5: git branch -D vs git branch --delete
    let git_branch_lower_d = "git branch -d old-branch";
    let git_branch_upper_d = "git branch -D old-branch";
    let git_branch_long = "git branch --delete old-branch";
    assert_eq!(detect_destructive_command(git_branch_lower_d), true);
    assert_eq!(detect_destructive_command(git_branch_upper_d), true); // Normalized to -d
    assert_eq!(
        detect_destructive_command(git_branch_long),
        false,
        "Documenting heuristic boundary: git branch --delete is not recognized, only -d"
    );
}
