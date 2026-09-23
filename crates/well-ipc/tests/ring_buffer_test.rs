use std::thread;
use well_ipc::ring_buffer::RingBuffer;

#[test]
fn test_ring_buffer_single_thread() {
    let rb = RingBuffer::<usize, 128>::new();
    let (producer, mut consumer) = rb.split();

    assert!(consumer.try_pop().is_none());

    producer.try_push(42).unwrap();
    assert_eq!(consumer.try_pop(), Some(42));
    assert!(consumer.try_pop().is_none());
}

#[test]
fn test_ring_buffer_multi_thread_stress() {
    let rb = RingBuffer::<usize, 1024>::new();
    let (producer, mut consumer) = rb.split();

    let items_to_send = 100_000;

    let producer_thread = thread::spawn(move || {
        for i in 0..items_to_send {
            while producer.try_push(i).is_err() {
                std::hint::spin_loop();
            }
        }
    });

    let mut received = 0;
    while received < items_to_send {
        if let Some(val) = consumer.try_pop() {
            assert_eq!(val, received);
            received += 1;
        } else {
            std::hint::spin_loop();
        }
    }

    producer_thread.join().unwrap();
}
