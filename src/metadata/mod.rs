mod spec;

// SparseIO shipped metadata trait example implementations.
#[cfg(feature = "metadata-memory")]
pub mod memory;

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io::Result;

pub(crate) use crate::common::chunks::{
    MAX_CHUNK_SIZE, chunk_index, chunk_offset, expected_chunk_len, expected_chunk_len_for_index,
};
pub(crate) use spec::checksum;
pub(crate) use spec::{ChunkRecord, MetadataSpec};

/// Trait describing how SparseIO stores metadata keys and values.
///
/// Implementations are intentionally dumb key/value stores. SparseIO owns the
/// meaning of every key, chunk mapping, and refcount entry layered on top.
pub trait Metadata: Send + Sync {
    /// Set a string key to an arbitrary value
    fn set<V>(&mut self, key: &str, value: V) -> impl std::future::Future<Output = std::io::Result<()>> + Send
    where
        V: Serialize + Send;

    /// Retrieve a typed object from the Metadata-defined KV store.
    ///
    /// Contract must ensure that the retrieved object is deserializable back
    /// to it's insertion type.
    fn get<V>(&self, key: &str) -> impl std::future::Future<Output = std::io::Result<Option<V>>> + Send
    where
        V: DeserializeOwned + Send;

    /// Prefix-based key search into Metadata-defined KV store.
    ///
    /// Contract must ensure that the retrieved object is deserializable back
    /// to it's insertion type.
    fn get_by_prefix<V>(
        &self,
        prefix: &str,
    ) -> impl std::future::Future<Output = std::io::Result<Vec<(String, V)>>> + Send
    where
        V: DeserializeOwned + Send;

    /// Removes a key from the defined KV store.
    fn delete(&mut self, key: &str) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}

/// Internal helper for verifying the validity of a SparseIO instance's
/// [`MetadataSpec`].
pub(crate) fn validate_spec(spec: &MetadataSpec) -> Result<()> {
    spec::validate_spec(spec)
}
