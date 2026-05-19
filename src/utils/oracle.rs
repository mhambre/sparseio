use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bytes::Bytes;
use tokio::sync::{Mutex, Notify};

/// In-memory oracle reader that returns exact fixture bytes.
#[derive(Clone)]
pub struct Reader {
    data: Arc<Vec<u8>>,
}

impl Reader {
    pub fn new(data: Bytes) -> Self {
        Self {
            data: Arc::new(data.to_vec()),
        }
    }
}

impl crate::Reader for Reader {
    async fn read_at(&self, offset: usize, buffer: &mut [u8]) -> std::io::Result<usize> {
        if offset >= self.data.len() || buffer.is_empty() {
            return Ok(0);
        }

        let end = (offset + buffer.len()).min(self.data.len());
        let src = &self.data[offset..end];
        buffer[..src.len()].copy_from_slice(src);
        Ok(src.len())
    }

    async fn len(&self) -> std::io::Result<usize> {
        Ok(self.data.len())
    }
}

/// In-memory oracle writer that stores exact cache payloads by key.
#[derive(Default, Clone)]
pub struct Writer {
    entries: Arc<Mutex<BTreeMap<String, Bytes>>>,
}

impl crate::Writer for Writer {
    async fn set_cache(&mut self, key: &str, value: &[u8]) -> std::io::Result<()> {
        self.entries.lock().await.insert(key.to_owned(), Bytes::copy_from_slice(value));
        Ok(())
    }

    async fn get_cache(&self, key: &str) -> std::io::Result<Option<Bytes>> {
        Ok(self.entries.lock().await.get(key).cloned())
    }

    async fn delete_cache(&mut self, key: &str) -> std::io::Result<()> {
        self.entries.lock().await.remove(key);
        Ok(())
    }
}

/// One-shot gate for pausing the next metadata mutation.
#[derive(Default, Clone)]
pub struct MetadataMutationGate {
    pending_pause: Arc<AtomicBool>,
    blocked: Arc<AtomicBool>,
    entered: Arc<Notify>,
    resume: Arc<Notify>,
}

impl MetadataMutationGate {
    pub fn pause_next_mutation(&self) {
        self.pending_pause.store(true, Ordering::SeqCst);
        self.blocked.store(false, Ordering::SeqCst);
    }

    pub async fn wait_until_blocked(&self) {
        while !self.blocked.load(Ordering::SeqCst) {
            self.entered.notified().await;
        }
    }

    pub fn resume(&self) {
        self.blocked.store(false, Ordering::SeqCst);
        self.resume.notify_waiters();
    }

    async fn pass_if_paused(&self) {
        if self.pending_pause.swap(false, Ordering::SeqCst) {
            let resume = self.resume.notified();
            self.blocked.store(true, Ordering::SeqCst);
            self.entered.notify_waiters();
            resume.await;
        }
    }
}

/// In-memory metadata oracle that only understands key/value storage.
#[derive(Clone, Default)]
pub struct Metadata {
    entries: Arc<Mutex<BTreeMap<String, Bytes>>>,
    mutation_gate: MetadataMutationGate,
}

impl Metadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mutation_gate(&self) -> MetadataMutationGate {
        self.mutation_gate.clone()
    }
}

impl crate::metadata::Metadata for Metadata {
    async fn set<V>(&mut self, key: &str, value: V) -> std::io::Result<()>
    where
        V: serde::Serialize + Send,
    {
        self.mutation_gate.pass_if_paused().await;
        self.entries
            .lock()
            .await
            .insert(key.to_owned(), crate::common::codec::encode_value(&value)?);
        Ok(())
    }

    async fn get<V>(&self, key: &str) -> std::io::Result<Option<V>>
    where
        V: serde::de::DeserializeOwned + Send,
    {
        self.entries
            .lock()
            .await
            .get(key)
            .map(|value| crate::common::codec::decode_value(value))
            .transpose()
    }

    async fn get_by_prefix<V>(&self, prefix: &str) -> std::io::Result<Vec<(String, V)>>
    where
        V: serde::de::DeserializeOwned + Send,
    {
        let mut entries = Vec::new();
        for (key, value) in self.entries.lock().await.iter() {
            if key.starts_with(prefix) {
                entries.push((key.clone(), crate::common::codec::decode_value(value)?));
            }
        }
        Ok(entries)
    }

    async fn delete(&mut self, key: &str) -> std::io::Result<()> {
        self.mutation_gate.pass_if_paused().await;
        self.entries.lock().await.remove(key);
        Ok(())
    }
}
