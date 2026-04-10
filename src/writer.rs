use bytes::Bytes;

/// Trait describing how SparseIO stores sparse extents.
///
/// Implementations must preserve exact byte fidelity for stored extents and
/// expose stable, contract-level behavior:
/// - `read_extent()` returns the exact bytes previously written at that offset.
/// - missing extents are represented as empty bytes.
/// - deleting a missing extent is a no-op.
/// - writing the same offset twice is last-write-wins.
///
/// Some examples include:
/// - A networked store that uses KV storage to store extents, allowing for distributed sparse objects.
/// - A file-backed store that uses an OS-provided sparse file <https://wiki.archlinux.org/title/Sparse_file>.
/// - A hybrid-store that caches hot areas of files to disk and less-frequently accessed portions to cheaper storage
///   like S3.
pub trait Writer: Send {
    /// Creates or overwrites the extent at `offset` with `data`.
    ///
    /// The bytes written must be readable back verbatim via `read_extent()`
    /// at the same offset. If the same offset is written more than once, the
    /// most recent write wins.
    fn create_extent(
        &mut self,
        offset: usize,
        data: bytes::Bytes,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;

    /// Reads the extent stored at `offset`.
    ///
    /// If no extent exists at the requested offset, implementations must
    /// return empty bytes rather than an error.
    fn read_extent(&self, offset: usize) -> impl std::future::Future<Output = std::io::Result<Bytes>> + Send;

    /// Deletes the extent stored at `offset`.
    ///
    /// Deleting a missing extent must succeed without error.
    fn delete_extent(&mut self, offset: usize) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}
