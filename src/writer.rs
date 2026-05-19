/// Trait describing how SparseIO stores cache payload bytes.
///
/// Implementations must preserve exact byte fidelity for stored payloads and
/// expose stable, contract-level behavior:
/// - `get_cache()` returns the exact bytes previously written at the same key.
/// - missing cache entries are represented as `None`.
/// - deleting a missing cache entry is a no-op.
/// - writing the same key twice is last-write-wins.
///
/// Some examples include:
/// - A networked store that uses KV storage to store cache payloads, allowing for distributed sparse objects.
/// - A file-backed store that uses a cache directory of content files.
/// - A hybrid-store that caches hot areas of files to disk and less-frequently accessed portions to cheaper storage
///   like S3.
pub trait Writer: Send {
    /// Creates or overwrites the payload stored at `key` with `value`.
    ///
    /// The bytes written must be readable back verbatim via `get_cache()` at
    /// the same key. If the same key is written more than once, the most
    /// recent write wins.
    fn set_cache(&mut self, key: &str, value: &[u8])
        -> impl std::future::Future<Output = std::io::Result<()>> + Send;

    /// Reads the payload stored at `key`.
    ///
    /// If no payload exists at the requested key, implementations must return
    /// `None` rather than an error.
    fn get_cache(&self, key: &str) -> impl std::future::Future<Output = std::io::Result<Option<bytes::Bytes>>> + Send;

    /// Deletes the payload stored at `key`.
    ///
    /// Deleting a missing cache payload must succeed without error.
    fn delete_cache(&mut self, key: &str) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}
