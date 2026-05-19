use std::io::Result;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::common::chunks::{MAX_CHUNK_SIZE, chunk_count_for_len, invalid_data};

pub(crate) const FORMAT_VERSION: u16 = 1; // Increment to denote breaking changes
const MAX_METADATA_BYTES: usize = 16 * 1024 * 1024; // 16 MiB, 128 KiB chunks ≈ ~65k chunks (64 GiB objects)

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
// Individual chunk record in the metadata spec, representing a single cached chunk of the
// object and its hash.
pub struct ChunkRecord {
    pub chunk_index: usize,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
// The encoded metadata format, which is what gets serialized and deserialized to recover the
// cached state of an object.
//
// **Note: MetadataSpecs are not portable across systems different with different bitnesses.
pub struct MetadataSpec {
    pub version: u16,
    pub source_identity: String,
    pub content_len: usize,
    pub chunk_size: usize,
    pub chunks: Vec<ChunkRecord>,
}

/// Computes the SHA-256 checksum of a given buffer.
pub fn checksum(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

pub fn validate_spec(spec: &MetadataSpec) -> Result<()> {
    validate_chunk_size(spec.chunk_size)?;
    validate_chunk_count(spec.chunk_size, spec.content_len, spec.chunks.len())?;

    let mut last = None;
    for chunk in &spec.chunks {
        validate_chunk_record(spec.chunk_size, spec.content_len, chunk)?;
        if last.is_some_and(|last| chunk.chunk_index <= last) {
            return Err(invalid_data("metadata chunks must be sorted and unique"));
        }
        last = Some(chunk.chunk_index);
    }

    Ok(())
}

/// Validates that the chunk size is within acceptable bounds
/// (greater than zero and less than or equal to the maximum).
fn validate_chunk_size(chunk_size: usize) -> Result<()> {
    if chunk_size == 0 {
        return Err(invalid_data("metadata chunk_size must be greater than zero"));
    }
    if chunk_size > MAX_CHUNK_SIZE {
        return Err(invalid_data("metadata chunk_size exceeds maximum"));
    }
    Ok(())
}

/// Validates that the number of chunks in the spec is sufficient to cover the content length of the
/// object, given the chunk size, and that there are no extraneous chunks beyond what would be needed
/// to cover the object.
fn validate_chunk_count(chunk_size: usize, object_len: usize, chunk_count: usize) -> Result<()> {
    let max_chunks: usize = chunk_count_for_len(chunk_size, object_len);
    if chunk_count > max_chunks {
        return Err(invalid_data("metadata chunk count exceeds object length"));
    }
    Ok(())
}

/// Validates that a chunk record is well-formed and within the bounds of the object as defined by
/// the content length and chunk size.
fn validate_chunk_record(chunk_size: usize, object_len: usize, chunk: &ChunkRecord) -> Result<()> {
    let max_chunks = chunk_count_for_len(chunk_size, object_len);
    if chunk.chunk_index >= max_chunks {
        return Err(invalid_data("metadata chunk index is at or beyond EOF"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::common::chunks::expected_chunk_len_for_index;

    #[test]
    fn final_chunk_length_is_derived_from_index() {
        assert_eq!(expected_chunk_len_for_index(16, 20, 1).expect("final chunk length should derive"), 4);
    }
}
