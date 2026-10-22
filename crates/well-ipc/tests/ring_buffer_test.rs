use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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
#[should_panic(expected = "ring buffer supports only one consumer")]
fn test_ring_buffer_rejects_multiple_consumers() {
    let rb = RingBuffer::<usize, 4>::new();
    let _first = rb.split();
    let _second = rb.split();
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

#[test]
fn test_ring_buffer_serializes_concurrent_producers_without_losing_items() {
    const PRODUCERS: usize = 4;
    const ITEMS_PER_PRODUCER: usize = 5_000;
    let rb = RingBuffer::<usize, 64>::new();
    let (producer, mut consumer) = rb.split();
    let producer = Arc::new(producer);
    let handles: Vec<_> = (0..PRODUCERS)
        .map(|producer_id| {
            let producer = Arc::clone(&producer);
            thread::spawn(move || {
                for sequence in 0..ITEMS_PER_PRODUCER {
                    let mut item = producer_id * ITEMS_PER_PRODUCER + sequence;
                    loop {
                        match producer.try_push(item) {
                            Ok(()) => break,
                            Err(returned) => {
                                item = returned;
                                thread::yield_now();
                            }
                        }
                    }
                }
            })
        })
        .collect();

    let total = PRODUCERS * ITEMS_PER_PRODUCER;
    let mut seen = vec![false; total];
    for _ in 0..total {
        let item = loop {
            if let Some(item) = consumer.try_pop() {
                break item;
            }
            thread::yield_now();
        };
        assert!(item < total, "producer emitted an out-of-range item");
        assert!(!seen[item], "producer item was duplicated");
        seen[item] = true;
    }

    for handle in handles {
        handle.join().unwrap();
    }
    assert!(seen.into_iter().all(|was_seen| was_seen));
}

#[derive(Clone, Debug)]
struct DropProbe(Arc<AtomicUsize>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn test_ring_buffer_drops_consumed_and_pending_values() {
    let drops = Arc::new(AtomicUsize::new(0));
    let rb = RingBuffer::<DropProbe, 2>::new();
    let (producer, mut consumer) = rb.split();

    producer.try_push(DropProbe(Arc::clone(&drops))).unwrap();
    let item = consumer.try_pop().unwrap();
    assert_eq!(drops.load(Ordering::Relaxed), 1);
    drop(item);
    assert_eq!(drops.load(Ordering::Relaxed), 2);

    producer.try_push(DropProbe(Arc::clone(&drops))).unwrap();
    producer.try_push(DropProbe(Arc::clone(&drops))).unwrap();
    drop(consumer);
    drop(producer);
    drop(rb);
    assert_eq!(drops.load(Ordering::Relaxed), 4);
}
