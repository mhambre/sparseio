use std::io::{Error, ErrorKind, Result};

/// Maximum chunk size accepted by SparseIO metadata and builders.
pub const MAX_CHUNK_SIZE: usize = 1024 * 1024 * 1024; // 1 GiB

/// Converts a chunk index into its absolute byte offset.
///
/// Returns an error if the multiplication would overflow `usize`.
pub fn chunk_offset(chunk_size: usize, chunk_index: usize) -> Result<usize> {
    chunk_index
        .checked_mul(chunk_size)
        .ok_or_else(|| invalid_data("metadata chunk offset exceeds usize"))
}

/// Converts an aligned byte offset into its chunk index.
///
/// Returns an error if `offset` is not aligned to `chunk_size`.
pub fn chunk_index(offset: usize, chunk_size: usize) -> Result<usize> {
    if !offset.is_multiple_of(chunk_size) {
        return Err(invalid_data("metadata chunk offset is not aligned to chunk_size"));
    }
    Ok(offset / chunk_size)
}

/// Returns the byte length of the chunk that starts at `offset`.
///
/// The final chunk of an object can be shorter than `chunk_size`.
pub(crate) fn expected_chunk_len(chunk_size: usize, object_len: usize, offset: usize) -> Result<usize> {
    if offset >= object_len {
        return Err(invalid_data("metadata chunk offset is at or beyond EOF"));
    }
    Ok(chunk_size.min(object_len - offset))
}

/// Returns the byte length of the chunk identified by `chunk_index`.
pub(crate) fn expected_chunk_len_for_index(chunk_size: usize, object_len: usize, chunk_index: usize) -> Result<usize> {
    expected_chunk_len(chunk_size, object_len, chunk_offset(chunk_size, chunk_index)?)
}

/// Returns the number of chunks required to cover an object of `object_len`.
pub(crate) fn chunk_count_for_len(chunk_size: usize, object_len: usize) -> usize {
    if object_len == 0 {
        0
    } else {
        object_len.div_ceil(chunk_size)
    }
}

pub(crate) fn invalid_data(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidData, message.into())
}
