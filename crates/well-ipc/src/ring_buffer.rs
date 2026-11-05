//! Bounded, owning multi-producer/multi-consumer transport.
//!
//! ArrayQueue owns each queued value exactly once. Async operations register
//! wakeups before checking the queue so concurrent consumers cannot lose signals.
use crossbeam_queue::ArrayQueue;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

pub struct RingBuffer<T, const N: usize> {
    queue: ArrayQueue<T>,
    readable: Notify,
    writable: Notify,
    producers: AtomicUsize,
    consumers: AtomicUsize,
}
impl<T, const N: usize> RingBuffer<T, N> {
    pub fn new() -> Arc<Self> {
        assert!(N > 0, "RingBuffer capacity must be greater than zero");
        Arc::new(Self {
            queue: ArrayQueue::new(N),
            readable: Notify::new(),
            writable: Notify::new(),
            producers: AtomicUsize::new(0),
            consumers: AtomicUsize::new(0),
        })
    }
    pub fn split(self: &Arc<Self>) -> (Producer<T, N>, Consumer<T, N>) {
        self.producers.fetch_add(1, Ordering::AcqRel);
        self.consumers.fetch_add(1, Ordering::AcqRel);
        (
            Producer {
                inner: Arc::clone(self),
            },
            Consumer {
                inner: Arc::clone(self),
            },
        )
    }
    pub fn len(&self) -> usize {
        self.queue.len()
    }
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
    pub fn capacity(&self) -> usize {
        N
    }
}
pub struct Producer<T, const N: usize> {
    inner: Arc<RingBuffer<T, N>>,
}
impl<T, const N: usize> Clone for Producer<T, N> {
    fn clone(&self) -> Self {
        self.inner.producers.fetch_add(1, Ordering::AcqRel);
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
impl<T, const N: usize> Drop for Producer<T, N> {
    fn drop(&mut self) {
        if self.inner.producers.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.inner.readable.notify_waiters();
        }
    }
}
impl<T, const N: usize> Producer<T, N> {
    pub fn is_closed(&self) -> bool {
        self.inner.consumers.load(Ordering::Acquire) == 0
    }
    pub fn try_push(&self, item: T) -> Result<(), T> {
        if self.is_closed() {
            return Err(item);
        }
        self.inner.queue.push(item)?;
        self.inner.readable.notify_one();
        Ok(())
    }
    /// Wait for capacity, returning the owned item if every consumer disconnects.
    pub async fn push(&self, mut item: T) -> Result<(), T> {
        loop {
            let notified = self.inner.writable.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.is_closed() {
                return Err(item);
            }
            match self.try_push(item) {
                Ok(()) => return Ok(()),
                Err(value) => item = value,
            }
            if self.is_closed() {
                return Err(item);
            }
            notified.await;
        }
    }
}
pub struct Consumer<T, const N: usize> {
    inner: Arc<RingBuffer<T, N>>,
}
impl<T, const N: usize> Clone for Consumer<T, N> {
    fn clone(&self) -> Self {
        self.inner.consumers.fetch_add(1, Ordering::AcqRel);
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
impl<T, const N: usize> Drop for Consumer<T, N> {
    fn drop(&mut self) {
        if self.inner.consumers.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.inner.writable.notify_waiters();
        }
    }
}
impl<T, const N: usize> Consumer<T, N> {
    pub fn is_closed(&self) -> bool {
        self.inner.producers.load(Ordering::Acquire) == 0
    }
    pub fn try_pop(&self) -> Option<T> {
        let item = self.inner.queue.pop()?;
        self.inner.writable.notify_one();
        Some(item)
    }
    /// Drain buffered items before reporting that every producer disconnected.
    pub async fn pop(&self) -> Option<T> {
        loop {
            let notified = self.inner.readable.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if let Some(item) = self.try_pop() {
                return Some(item);
            }
            if self.is_closed() {
                // A producer may have published just before dropping its handle.
                return self.try_pop();
            }
            notified.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::time::Duration;

    #[test]
    #[should_panic(expected = "greater than zero")]
    fn rejects_zero_capacity() {
        RingBuffer::<u8, 0>::new();
    }

    #[test]
    fn values_move_once_and_unconsumed_values_are_dropped() {
        struct Value(Arc<AtomicUsize>);
        impl Drop for Value {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let drops = Arc::new(AtomicUsize::new(0));
        {
            let ring = RingBuffer::<Value, 2>::new();
            let (p, c) = ring.split();
            assert!(p.try_push(Value(drops.clone())).is_ok());
            assert!(p.try_push(Value(drops.clone())).is_ok());
            drop(p.try_push(Value(drops.clone())).err().unwrap());
            assert_eq!(drops.load(Ordering::Relaxed), 1);
            drop(c.try_pop().unwrap());
            assert_eq!(drops.load(Ordering::Relaxed), 2);
        }
        assert_eq!(drops.load(Ordering::Relaxed), 3);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_producers_and_consumers_do_not_lose_or_duplicate_items() {
        let ring = RingBuffer::<usize, 17>::new();
        let (p, c) = ring.split();
        let mut producers = Vec::new();
        for writer in 0..4 {
            let p = p.clone();
            producers.push(tokio::spawn(async move {
                for i in 0..2000 {
                    p.push(writer * 2000 + i).await.unwrap();
                }
            }));
        }
        drop(p);
        let mut readers = Vec::new();
        for _ in 0..4 {
            let c = c.clone();
            readers.push(tokio::spawn(async move {
                let mut received = Vec::new();
                while let Some(value) = c.pop().await {
                    received.push(value);
                }
                received
            }));
        }
        drop(c);
        let run = async {
            for writer in producers {
                writer.await.unwrap();
            }
            let mut seen = HashSet::new();
            for reader in readers {
                for value in reader.await.unwrap() {
                    assert!(seen.insert(value), "duplicate {value}");
                }
            }
            assert_eq!(seen.len(), 8000);
        };
        tokio::time::timeout(Duration::from_secs(5), run)
            .await
            .expect("queue stalled");
    }
    #[tokio::test]
    async fn disconnect_wakes_waiters_and_buffer_drains_in_order() {
        let ring = RingBuffer::<usize, 1>::new();
        let (p, c) = ring.split();
        p.push(1).await.unwrap();
        let waiting = tokio::spawn(async move { p.push(2).await });
        tokio::task::yield_now().await;
        drop(c);
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), waiting)
                .await
                .unwrap()
                .unwrap(),
            Err(2)
        );
        let (p, c) = ring.split();
        assert_eq!(c.pop().await, Some(1));
        p.push(3).await.unwrap();
        drop(p);
        assert_eq!(c.pop().await, Some(3));
        assert_eq!(c.pop().await, None);
    }
}
