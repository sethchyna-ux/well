use std::thread;
use well_ipc::ring_buffer::RingBuffer;
use well_ipc::TypedBlock;

const ITERATIONS: usize = 10_000;
const BUFFER_CAPACITY: usize = 4096;

#[test]
fn test_ring_buffer_stress() {
    let ring_buffer = RingBuffer::<TypedBlock, BUFFER_CAPACITY>::new();
    let (producer, mut consumer) = ring_buffer.split();

    // The current ring buffer is single-producer/single-consumer. Emit both
    // stream types from one producer thread to exercise render-side consumption
    // without relying on unsupported multi-producer behavior.
    let handle = thread::spawn(move || {
        for i in 0..ITERATIONS {
            let stdout = TypedBlock::StdoutChunk(format!("Stdout Iteration {}", i).into_bytes());
            while producer.try_push(stdout.clone()).is_err() {
                std::hint::spin_loop();
            }

            let stderr = TypedBlock::StderrChunk(format!("Stderr Iteration {}", i).into_bytes());
            while producer.try_push(stderr.clone()).is_err() {
                std::hint::spin_loop();
            }
        }
    });

    // Consumer thread simulating the Hephaestus engine
    let mut received_stdout = 0;
    let mut received_stderr = 0;

    let expected_total = ITERATIONS * 2;

    while received_stdout + received_stderr < expected_total {
        if let Some(block) = consumer.try_pop() {
            match block {
                TypedBlock::StdoutChunk(_) => received_stdout += 1,
                TypedBlock::StderrChunk(_) => received_stderr += 1,
                _ => panic!("Unexpected block type"),
            }
        } else {
            std::hint::spin_loop();
        }
    }

    handle.join().unwrap();

    assert_eq!(received_stdout, ITERATIONS);
    assert_eq!(received_stderr, ITERATIONS);
}
