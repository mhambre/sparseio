//! User-facing traits for plugging storage backends into SparseIO.

use std::io;

use async_trait::async_trait;
use bytes::Bytes;

/// Reads explicit byte ranges from an upstream data source.
#[async_trait]
#[allow(clippy::len_without_is_empty)]
pub trait Reader: Send + Sync {
    /// Return the total number of bytes in the source object.
    async fn len(&self) -> io::Result<usize>;

    /// Read `length` bytes beginning at `offset`.
    async fn read_at(&self, offset: usize, length: usize) -> io::Result<Bytes>;
}

/// Stores and retrieves cached byte ranges.
#[async_trait]
pub trait Writer: Send + Sync {
    /// Write bytes into the cache under `key`.
    async fn write(&self, key: &str, value: Bytes) -> io::Result<()>;

    /// Read a byte range from the cache.
    async fn read(&self, key: &str) -> io::Result<Option<Bytes>>;

    /// Delete all cached data associated with `key`.
    async fn delete(&self, key: &str) -> io::Result<()>;
}

/// Key-value metadata store used for coverage maps and CAS reference counts.
#[async_trait]
pub trait Metadata: Send + Sync {
    /// Retrieve a value from the metadata store.
    async fn get(&self, key: &str) -> io::Result<Option<Bytes>>;

    /// Persist a value in the metadata store.
    async fn set(&self, key: &str, value: Bytes) -> io::Result<()>;

    /// Delete a value from the metadata store.
    async fn delete(&self, key: &str) -> io::Result<()>;
}
