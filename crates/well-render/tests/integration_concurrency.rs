use std::sync::Arc;
use std::thread;
use std::time::Duration;
use well_ipc::ring_buffer::RingBuffer;
use well_ipc::TypedBlock;

const ITERATIONS: usize = 1_000_000;
const BUFFER_CAPACITY: usize = 4096;

#[test]
fn test_ring_buffer_stress() {
    let ring_buffer = RingBuffer::<TypedBlock, BUFFER_CAPACITY>::new();
    let (producer, mut consumer) = ring_buffer.split();
    
    let producer_arc = Arc::new(producer);

    // Spawn producer threads to simulate stdout and stderr streams
    let mut handles = vec![];
    
    for thread_id in 0..2 {
        let p = Arc::clone(&producer_arc);
        handles.push(thread::spawn(move || {
            for i in 0..ITERATIONS {
                let payload = format!("Thread {} Iteration {}", thread_id, i).into_bytes();
                let block = if thread_id == 0 {
                    TypedBlock::StdoutChunk(payload)
                } else {
                    TypedBlock::StderrChunk(payload)
                };
                
                while let Err(_) = p.try_push(block.clone()) {
                    std::hint::spin_loop();
                }
            }
        }));
    }

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

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(received_stdout, ITERATIONS);
    assert_eq!(received_stderr, ITERATIONS);
}
