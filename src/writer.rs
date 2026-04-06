use bytes::Bytes;

/// User-implemented trait for how sparse extents are stored and retrieved. Examples include:
/// - A simple in-memory store for testing and prototyping.
/// - A networked store that uses KV storage to store extents, allowing for distributed sparse objects.
/// - A file-backed store that uses an OS provided sparse file https://wiki.archlinux.org/title/Sparse_file.
/// - A file-backed store that uses shards of a file as extents.
pub trait Writer: Send {
    /// Create a new extent at the given offset with the provided data. Offset will be aligned
    /// to the chunk size, will write the provided data at the given offset, and update internal
    /// metadata to reflect the new extent.
    fn create_extent(
        &mut self,
        offset: usize,
        data: bytes::Bytes,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;

    /// Finds and Reads an existing extent at the given offset, if it exists. Offset will be aligned to the chunk size.
    fn read_extent(&self, offset: usize) -> impl std::future::Future<Output = std::io::Result<Option<Bytes>>> + Send;

    /// Searches for an existing extent metadata for the given offset. Offset will be aligned to the chunk size.
    fn search_extent(&self, offset: usize)
    -> impl std::future::Future<Output = std::io::Result<Option<Extent>>> + Send;

    /// Removes an existing extent metadata, and data, at the given offset, if it exists. Offset will be aligned to the chunk size.
    fn delete_extent(&mut self, offset: usize) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}

// An extent represents a physical range of bytes in a sparse object.
// (i.e. the filled in holes of a sparse file on linux). Alignment
// allows us to determine if logical chunks can be coallesced.
pub struct Extent {
    pub offset: usize,
    pub length: usize,
}
