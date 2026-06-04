//! User-facing traits for plugging storage backends into SparseIO.

use std::io;

use bytes::Bytes;

/// Reads explicit byte ranges from an upstream data source.
#[allow(clippy::len_without_is_empty)]
pub trait Reader: Send + Sync {
    /// Return the total number of bytes in the source object.
    fn len(&self) -> io::Result<usize>;

    /// Read `length` bytes beginning at `offset`.
    fn read_at(&self, offset: usize, length: usize) -> io::Result<Bytes>;
}

/// Stores and retrieves cached byte ranges.
pub trait Writer: Send + Sync {
    /// Write bytes into the cache under `key`.
    fn write(&self, key: &str) -> io::Result<()>;

    /// Read a byte range from the cache.
    fn read(&self, key: &str) -> io::Result<Option<Bytes>>;

    /// Delete all cached data associated with `key`.
    fn delete(&self, key: &str) -> io::Result<()>;
}

/// Key-value metadata store used for coverage maps and CAS reference counts.
pub trait Metadata: Send + Sync {
    /// Retrieve a value from the metadata store.
    fn get(&self, key: &str) -> io::Result<Option<Bytes>>;

    /// Persist a value in the metadata store.
    fn set(&self, key: &str, value: Bytes) -> io::Result<()>;

    /// Delete a value from the metadata store.
    fn delete(&self, key: &str) -> io::Result<()>;
}
