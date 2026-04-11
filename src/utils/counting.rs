use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use bytes::Bytes;
use tokio::sync::Mutex;
use tokio::time::sleep;

/// Counts reader calls while delegating all behavior to an inner reader.
///
/// The wrapper lets tests and examples assert upstream read and length probe
/// counts without changing the wrapped reader's observable bytes.
pub struct Reader<R> {
    inner: Arc<R>,
    reads: Arc<AtomicUsize>,
    len_reads: Arc<AtomicUsize>,
    read_delay: Option<Duration>,
}

impl<R> Clone for Reader<R> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            reads: Arc::clone(&self.reads),
            len_reads: Arc::clone(&self.len_reads),
            read_delay: self.read_delay,
        }
    }
}

impl<R> Reader<R> {
    /// Wraps a reader with call counters and zero-initialized totals.
    pub fn new(inner: R) -> Self {
        Self {
            inner: Arc::new(inner),
            reads: Arc::new(AtomicUsize::new(0)),
            len_reads: Arc::new(AtomicUsize::new(0)),
            read_delay: None,
        }
    }

    /// Returns a copy of this reader that sleeps before each `read_at` call.
    ///
    /// This is useful when tests need to force overlapping in-flight reads so
    /// dedupe behavior is exercised deterministically.
    pub fn with_read_delay(mut self, delay: Duration) -> Self {
        self.read_delay = Some(delay);
        self
    }

    /// Returns the number of `read_at` calls observed so far.
    pub fn read_count(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    /// Returns the number of `len` calls observed so far.
    pub fn len_read_count(&self) -> usize {
        self.len_reads.load(Ordering::SeqCst)
    }
}

impl<R> crate::Reader for Reader<R>
where
    R: crate::Reader + Send + Sync + 'static,
{
    async fn read_at(&self, offset: usize, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if let Some(delay) = self.read_delay {
            sleep(delay).await;
        }
        self.inner.read_at(offset, buffer).await
    }

    async fn len(&self) -> std::io::Result<usize> {
        self.len_reads.fetch_add(1, Ordering::SeqCst);
        self.inner.len().await
    }
}

/// Counts writer calls while delegating all behavior to an inner writer.
///
/// The wrapper lets tests and examples verify how many times SparseIO
/// materializes, re-reads, or deletes extents without altering the wrapped
/// writer's storage semantics.
pub struct Writer<W> {
    inner: Arc<Mutex<W>>,
    creates: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
    deletes: Arc<AtomicUsize>,
}

impl<W> Clone for Writer<W> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            creates: Arc::clone(&self.creates),
            reads: Arc::clone(&self.reads),
            deletes: Arc::clone(&self.deletes),
        }
    }
}

impl<W> Writer<W> {
    /// Wraps a writer with call counters and zero-initialized totals.
    pub fn new(inner: W) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
            creates: Arc::new(AtomicUsize::new(0)),
            reads: Arc::new(AtomicUsize::new(0)),
            deletes: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the number of `create_extent` calls observed so far.
    pub fn create_count(&self) -> usize {
        self.creates.load(Ordering::SeqCst)
    }

    /// Returns the number of `read_extent` calls observed so far.
    pub fn read_count(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    /// Returns the number of `delete_extent` calls observed so far.
    pub fn delete_count(&self) -> usize {
        self.deletes.load(Ordering::SeqCst)
    }
}

impl<W> crate::Writer for Writer<W>
where
    W: crate::Writer + Send + Sync + 'static,
{
    async fn create_extent(&mut self, offset: usize, data: Bytes) -> std::io::Result<()> {
        self.creates.fetch_add(1, Ordering::SeqCst);
        self.inner.lock().await.create_extent(offset, data).await
    }

    async fn read_extent(&self, offset: usize) -> std::io::Result<Bytes> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.lock().await.read_extent(offset).await
    }

    async fn delete_extent(&mut self, offset: usize) -> std::io::Result<()> {
        self.deletes.fetch_add(1, Ordering::SeqCst);
        self.inner.lock().await.delete_extent(offset).await
    }
}
