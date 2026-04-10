use bytes::Bytes;

/// Trait defining how to use a particular storage backend as a sparse cache for retrieved extents
/// of objects.
///
/// Some examples include:
/// - A networked store that uses KV storage to store extents, allowing for distributed sparse objects.
/// - A file-backed store that uses an OS-provided sparse file <https://wiki.archlinux.org/title/Sparse_file>.
/// - A hybrid-store that caches hot areas of files to disk and less-frequently accessed portions to cheaper storage like S3.
pub trait Writer: Send {
    /// Create a new extent at the given offset with the provided data. Offset will be aligned
    /// to the chunk size, will write the provided data at the given offset, and update internal
    /// metadata to reflect the new extent.
    fn create_extent(
        &mut self,
        offset: usize,
        data: bytes::Bytes,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;

    /// Finds/validates existence of, and reads an existing extent at the given offset, if it exists.
    /// Offset will be aligned to the chunk size prior. Returns the extent contents.
    fn read_extent(&self, offset: usize) -> impl std::future::Future<Output = std::io::Result<Bytes>> + Send;

    /// Removes an existing extent metadata, and data, at the given offset, if it exists. Offset will
    /// be aligned to the chunk size prior.
    fn delete_extent(&mut self, offset: usize) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}
