use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use well_ipc::ring_buffer::RingBuffer;
use well_ipc::seqlock::HermesSeqlock;

#[derive(Clone, Copy, Default)]
#[allow(dead_code)]
struct CellState {
    seq: u64,
    x: u32,
    y: u32,
    ch: char,
}

fn bench_seqlock_read_write(c: &mut Criterion) {
    c.bench_function("seqlock_uncontended_write_read", |b| {
        let lock = HermesSeqlock::new(CellState {
            seq: 1,
            x: 80,
            y: 24,
            ch: 'W',
        });
        let mut seq = 1u64;
        b.iter(|| {
            seq += 1;
            lock.write(CellState {
                seq,
                x: 80,
                y: 24,
                ch: 'A',
            });
            let read_val = lock.read();
            black_box(read_val);
        });
    });

    c.bench_function("seqlock_reader_under_writer_thread", |b| {
        let lock = Arc::new(HermesSeqlock::new(CellState::default()));
        let running = Arc::new(AtomicBool::new(true));

        let writer_lock = lock.clone();
        let writer_running = running.clone();
        let writer_handle = thread::spawn(move || {
            let mut i = 0u64;
            while writer_running.load(Ordering::Relaxed) {
                writer_lock.write(CellState {
                    seq: i,
                    x: (i % 80) as u32,
                    y: (i % 24) as u32,
                    ch: 'X',
                });
                i += 1;
            }
        });

        b.iter(|| {
            let state = lock.read();
            black_box(state);
        });

        running.store(false, Ordering::Relaxed);
        let _ = writer_handle.join();
    });
}

fn bench_spsc_ring_buffer(c: &mut Criterion) {
    c.bench_function("spsc_ring_buffer_push_pop", |b| {
        let ring = RingBuffer::<u64, 1024>::new();
        let (prod, cons) = ring.split();
        b.iter(|| {
            for i in 0..100 {
                let _ = prod.try_push(black_box(i));
            }
            while let Some(val) = cons.try_pop() {
                black_box(val);
            }
        });
    });
}

criterion_group!(benches, bench_seqlock_read_write, bench_spsc_ring_buffer);
criterion_main!(benches);
