use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// A bounded queue shared by a producer and consumer.
///
/// Access is synchronized so producers may safely call `try_push` concurrently.
pub struct RingBuffer<T, const N: usize> {
    buffer: Mutex<VecDeque<T>>,
    not_full: Notify,
}

impl<T, const N: usize> RingBuffer<T, N> {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            buffer: Mutex::new(VecDeque::new()),
            not_full: Notify::new(),
        })
    }

    pub fn split(self: &Arc<Self>) -> (Producer<T, N>, Consumer<T, N>) {
        (
            Producer {
                inner: Arc::clone(self),
            },
            Consumer {
                inner: Arc::clone(self),
            },
        )
    }
}

pub struct Producer<T, const N: usize> {
    inner: Arc<RingBuffer<T, N>>,
}

impl<T, const N: usize> Producer<T, N> {
    /// Attempt to enqueue an item. Returns the item unchanged when the queue is full.
    pub fn try_push(&self, item: T) -> Result<(), T> {
        let mut buffer = self.inner.buffer.lock().expect("ring buffer lock poisoned");
        if buffer.len() >= N {
            return Err(item);
        }
        buffer.push_back(item);
        Ok(())
    }

    /// Enqueue an item, waiting asynchronously until the consumer frees capacity.
    pub async fn push(&self, mut item: T) -> Result<(), T> {
        if N == 0 {
            return Err(item);
        }
        loop {
            let notified = self.inner.not_full.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            match self.try_push(item) {
                Ok(()) => return Ok(()),
                Err(returned) => item = returned,
            }
            notified.await;
        }
    }
}

pub struct Consumer<T, const N: usize> {
    inner: Arc<RingBuffer<T, N>>,
}

impl<T, const N: usize> Consumer<T, N> {
    /// Remove and return the oldest queued item, if any.
    pub fn try_pop(&mut self) -> Option<T> {
        let item = self
            .inner
            .buffer
            .lock()
            .expect("ring buffer lock poisoned")
            .pop_front();
        if item.is_some() {
            self.inner.not_full.notify_one();
        }
        item
    }
}

#[cfg(test)]
mod tests {
    use super::RingBuffer;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn preserves_fifo_and_returns_item_when_full() {
        let queue = RingBuffer::<_, 2>::new();
        let (producer, mut consumer) = queue.split();

        assert_eq!(producer.try_push("first"), Ok(()));
        assert_eq!(producer.try_push("second"), Ok(()));
        assert_eq!(producer.try_push("overflow"), Err("overflow"));
        assert_eq!(consumer.try_pop(), Some("first"));
        assert_eq!(consumer.try_pop(), Some("second"));
        assert_eq!(consumer.try_pop(), None);
    }

    #[test]
    fn supports_non_clone_items_and_zero_capacity() {
        let queue = RingBuffer::<_, 1>::new();
        let (producer, mut consumer) = queue.split();
        assert_eq!(producer.try_push(Box::new(7)), Ok(()));
        assert_eq!(*consumer.try_pop().unwrap(), 7);

        let empty = RingBuffer::<u8, 0>::new();
        let (producer, mut consumer) = empty.split();
        assert_eq!(producer.try_push(1), Err(1));
        assert_eq!(consumer.try_pop(), None);
    }

    #[test]
    fn concurrent_producers_do_not_lose_accepted_items() {
        const PRODUCERS: usize = 4;
        const ITEMS_PER_PRODUCER: usize = 100;
        const CAPACITY: usize = PRODUCERS * ITEMS_PER_PRODUCER;

        let queue = RingBuffer::<usize, CAPACITY>::new();
        let (producer, mut consumer) = queue.split();
        let producer = Arc::new(producer);
        let threads: Vec<_> = (0..PRODUCERS)
            .map(|producer_id| {
                let producer = Arc::clone(&producer);
                thread::spawn(move || {
                    for item in 0..ITEMS_PER_PRODUCER {
                        producer
                            .try_push(producer_id * ITEMS_PER_PRODUCER + item)
                            .expect("queue has sufficient capacity");
                    }
                })
            })
            .collect();

        for thread in threads {
            thread.join().unwrap();
        }

        let mut received = Vec::new();
        while let Some(item) = consumer.try_pop() {
            received.push(item);
        }
        received.sort_unstable();
        assert_eq!(
            received,
            (0..CAPACITY).collect::<Vec<_>>(),
            "every accepted item should be delivered exactly once"
        );
    }

    #[tokio::test]
    async fn async_push_waits_for_capacity_without_losing_wakeup() {
        let queue = RingBuffer::<_, 1>::new();
        let (producer, mut consumer) = queue.split();
        producer.try_push("first").unwrap();

        let blocked = tokio::spawn(async move {
            producer.push("second").await.unwrap();
        });
        tokio::task::yield_now().await;
        assert_eq!(consumer.try_pop(), Some("first"));
        blocked.await.unwrap();
        assert_eq!(consumer.try_pop(), Some("second"));
    }
}
