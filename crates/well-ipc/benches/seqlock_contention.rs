//! Micro-benchmark suite for Hermes Seqlock contention and Shared-Memory Bypass Ring throughput.

use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use well_ipc::{HermesSeqlock, HermesViewportState, SharedMemoryBypassRing};

fn main() {
    let seqlock = Arc::new(HermesSeqlock::new(HermesViewportState::default()));
    let iterations_per_thread = 100_000u64;

    let start = Instant::now();
    std::thread::scope(|s| {
        let writer_lock = Arc::clone(&seqlock);
        s.spawn(move || {
            for seq in 0..iterations_per_thread {
                let mut state = HermesViewportState::default();
                state.cursor_row = (seq % 80) as u16;
                state.cursor_col = (seq % 120) as u16;
                state.generation = seq;
                writer_lock.write(state);
            }
        });

        for _ in 0..4 {
            let reader_lock = Arc::clone(&seqlock);
            s.spawn(move || {
                for _ in 0..iterations_per_thread {
                    black_box(reader_lock.read());
                }
            });
        }
    });
    let seqlock_elapsed = start.elapsed();
    let total_reads = iterations_per_thread * 4;
    let ns_per_read = seqlock_elapsed.as_nanos() / (total_reads as u128);

    // Benchmark SharedMemoryBypassRing throughput (64 MiB stream)
    let ring = Arc::new(SharedMemoryBypassRing::new(4 * 1024 * 1024).expect("mmap ring"));
    let total_bytes: usize = 64 * 1024 * 1024;
    let shm_start = Instant::now();
    std::thread::scope(|s| {
        let writer_ring = Arc::clone(&ring);
        s.spawn(move || {
            let chunk = vec![0x5Au8; 64 * 1024];
            let mut sent = 0usize;
            while sent < total_bytes {
                let rem = (total_bytes - sent).min(chunk.len());
                let n = writer_ring.write_chunk(&chunk[..rem]);
                if n == 0 {
                    std::thread::yield_now();
                } else {
                    sent += n;
                }
            }
        });

        let mut buf = vec![0u8; 64 * 1024];
        let mut received = 0usize;
        while received < total_bytes {
            let n = ring.read_chunk(&mut buf);
            if n == 0 {
                std::thread::yield_now();
            } else {
                received += n;
            }
        }
    });
    let shm_elapsed = shm_start.elapsed();
    let gib_per_sec =
        (total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)) / shm_elapsed.as_secs_f64();

    let summary = format!(
        "=== Hermes Seqlock & Shared-Memory Ring Benchmark ===\nseqlock_reads={total_reads} elapsed={seqlock_elapsed:?} mean_read_ns={ns_per_read}ns\nshm_bytes={total_bytes} elapsed={shm_elapsed:?} throughput_gib_s={gib_per_sec:.2} GiB/s\n"
    );
    print!("{summary}");

    let criterion_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("target/criterion/seqlock_contention"))
        .unwrap_or_else(|| PathBuf::from("target/criterion/seqlock_contention"));
    let _ = fs::create_dir_all(&criterion_dir);
    let _ = fs::write(criterion_dir.join("summary.txt"), summary);
}
