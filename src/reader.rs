/// Trait to describe the Input operations for retrieving data from the associated upstream. This defines how
/// the library fetches data from the source,
pub trait Reader: Send {
    /// Implementation of how to read a chunk of data from the source at the
    /// given offset.
    fn read_at(
        &self,
        offset: usize,
        buffer: &mut [u8],
    ) -> impl std::future::Future<Output = std::io::Result<usize>> + Send;

    /// Get the length of the full object.
    fn len(&self) -> impl std::future::Future<Output = usize> + Send;
}
