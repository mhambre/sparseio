use bytes::Bytes;

/// Generates deterministic fixture bytes used across examples and tests.
///
/// The byte pattern is stable and nontrivial so callers can compare exact
/// slices without relying on random data.
pub fn bytes(len: usize) -> Bytes {
    let mut data = vec![0u8; len];
    for (index, byte) in data.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    Bytes::from(data)
}

/// Returns chunk-aligned offsets covering `len` bytes.
///
/// This is useful when a caller needs to touch each materialized chunk exactly
/// once without assuming anything about internal storage segmentation.
pub fn chunk_offsets(len: usize, chunk_size: usize) -> Vec<usize> {
    (0..len).step_by(chunk_size.max(1)).collect()
}
