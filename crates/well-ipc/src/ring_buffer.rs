use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{fence, AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// An atomic sequence lock for zero-copy synchronization.
pub struct Seqlock {
    seq: AtomicUsize,
}

impl Seqlock {
    pub fn new() -> Self {
        Self {
            seq: AtomicUsize::new(0),
        }
    }

    /// Mark the beginning of a write.
    pub fn begin_write(&self) -> usize {
        let seq = self.seq.load(Ordering::Relaxed);
        self.seq.store(seq + 1, Ordering::Release);
        seq + 1
    }

    /// Mark the end of a write.
    pub fn end_write(&self) -> usize {
        let seq = self.seq.load(Ordering::Relaxed);
        self.seq.store(seq + 1, Ordering::Release);
        seq + 1
    }

    /// Read the sequence number before accessing data.
    pub fn begin_read(&self) -> usize {
        self.seq.load(Ordering::Acquire)
    }

    /// Validate the sequence number after reading data.
    /// Returns true if the data was not modified concurrently.
    pub fn end_read(&self, start_seq: usize) -> bool {
        fence(Ordering::Acquire);
        let end_seq = self.seq.load(Ordering::Relaxed);
        start_seq == end_seq && (start_seq & 1) == 0
    }
}

/// A bounded ring buffer with serialized producers and a lock-free consumer.
pub struct RingBuffer<T, const N: usize> {
    buffer: UnsafeCell<[MaybeUninit<T>; N]>,
    seqlocks: [Seqlock; N],
    head: AtomicUsize,
    tail: AtomicUsize,
    producer_lock: Mutex<()>,
    consumer_created: AtomicBool,
}

unsafe impl<T: Send, const N: usize> Send for RingBuffer<T, N> {}
unsafe impl<T: Sync, const N: usize> Sync for RingBuffer<T, N> {}

impl<T, const N: usize> RingBuffer<T, N> {
    pub fn new() -> Arc<Self> {
        assert!(N > 0, "ring buffer capacity must be greater than zero");
        let seqlocks = std::array::from_fn(|_| Seqlock::new());
        Arc::new(Self {
            buffer: UnsafeCell::new(unsafe { MaybeUninit::uninit().assume_init() }),
            seqlocks,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            producer_lock: Mutex::new(()),
            consumer_created: AtomicBool::new(false),
        })
    }

    /// Create producer handles and the ring buffer's single consumer.
    pub fn split(self: &Arc<Self>) -> (Producer<T, N>, Consumer<T, N>) {
        assert!(
            self.consumer_created
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok(),
            "ring buffer supports only one consumer"
        );
        (
            Producer {
                inner: Arc::clone(self),
            },
            Consumer {
                inner: Arc::clone(self),
                local_tail: 0,
            },
        )
    }
}

impl<T, const N: usize> Drop for RingBuffer<T, N> {
    fn drop(&mut self) {
        let mut tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);
        while tail != head {
            let idx = tail % N;
            unsafe {
                (*self.buffer.get())[idx].assume_init_drop();
            }
            tail = tail.wrapping_add(1);
        }
    }
}

pub struct Producer<T, const N: usize> {
    inner: Arc<RingBuffer<T, N>>,
}

impl<T, const N: usize> Producer<T, N> {
    /// Attempt to push an item into the buffer.
    /// Returns Err(item) if the buffer is full.
    pub fn try_push(&self, item: T) -> Result<(), T> {
        let _guard = self
            .inner
            .producer_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let head = self.inner.head.load(Ordering::Relaxed);
        let tail = self.inner.tail.load(Ordering::Acquire);

        if head.wrapping_sub(tail) >= N {
            return Err(item);
        }

        let idx = head % N;
        let seqlock = &self.inner.seqlocks[idx];

        seqlock.begin_write();
        unsafe {
            (*self.inner.buffer.get())[idx] = MaybeUninit::new(item);
        }
        seqlock.end_write();

        self.inner
            .head
            .store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }
}

pub struct Consumer<T, const N: usize> {
    inner: Arc<RingBuffer<T, N>>,
    local_tail: usize,
}

impl<T: Clone, const N: usize> Consumer<T, N> {
    /// Attempt to pop an item from the buffer.
    pub fn try_pop(&mut self) -> Option<T> {
        let head = self.inner.head.load(Ordering::Acquire);

        if self.local_tail == head {
            return None;
        }

        let idx = self.local_tail % N;
        let seqlock = &self.inner.seqlocks[idx];

        let mut retries = 0;
        let item = loop {
            let seq = seqlock.begin_read();

            // If seq is odd, a writer is currently modifying this slot.
            if seq & 1 != 0 {
                std::hint::spin_loop();
                continue;
            }

            let item = unsafe { (*self.inner.buffer.get())[idx].assume_init_ref().clone() };

            if seqlock.end_read(seq) {
                break item;
            }

            retries += 1;
            if retries > 1000 {
                // Highly contended or torn; fallback handling in real impl might be needed,
                // but this loop usually succeeds quickly.
                std::hint::spin_loop();
            }
        };

        unsafe {
            (*self.inner.buffer.get())[idx].assume_init_drop();
        }
        self.local_tail = self.local_tail.wrapping_add(1);
        self.inner.tail.store(self.local_tail, Ordering::Release);

        Some(item)
    }
}
