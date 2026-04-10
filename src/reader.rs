/// Trait describing how SparseIO reads bytes from an upstream source.
///
/// Implementations must provide a stable `len()` for the lifetime of the
/// reader, exact byte fidelity for each readable range, and EOF semantics that
/// are consistent across repeated calls:
/// - `len()` must return the same length unless the underlying object itself changes.
/// - `read_at()` must copy the exact bytes that exist at the requested offset.
/// - reads at EOF or beyond EOF must return `Ok(0)`.
/// - non-empty reads that start before EOF must make forward progress.
pub trait Reader: Send {
    /// Reads bytes starting at `offset` into `buffer`.
    ///
    /// Implementations should return the number of bytes copied into `buffer`.
    /// When `offset` is before EOF and `buffer` is non-empty, the call must
    /// return at least one byte unless the source is empty.
    fn read_at(
        &self,
        offset: usize,
        buffer: &mut [u8],
    ) -> impl std::future::Future<Output = std::io::Result<usize>> + Send;

    /// Returns the length of the full logical object.
    ///
    /// The value must be stable for the lifetime of the reader unless the
    /// underlying source itself mutates.
    fn len(&self) -> impl std::future::Future<Output = std::io::Result<usize>> + Send;
}
