/// User-implemented trait for how sparse extents are stored and retrieved. Examples include:
/// - A simple in-memory store for testing and prototyping.
/// - A networked store that uses KV storage to store extents, allowing for distributed sparse objects.
/// - A file-backed store that uses an OS provided sparse file https://wiki.archlinux.org/title/Sparse_file.
/// - A file-backed store that uses shards of a file as extents.
#[allow(async_fn_in_trait)]
pub trait ExtentStore {
    async fn create_extent(&mut self, offset: usize, data: bytes::Bytes) -> std::io::Result<()>;
    async fn read_extent(&self, offset: usize) -> std::io::Result<Option<Extent>>;
    async fn delete_extent(&mut self, offset: usize) -> std::io::Result<()>;
}

// An extent represents a physical range of bytes in a sparse object.
// (i.e. the filled in holes of a sparse file on linux). Alignment
// allows us to determine if logical chunks can be coallesced.
pub struct Extent {
    pub offset: usize,
    pub length: usize,
}
