use std::collections::HashSet;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Verifies that a fully materialized destination file exactly matches the source file.
///
/// This is the final-state check used by the file-to-file example and the file-backed
/// integration test. It intentionally compares whole-file bytes so the assertion tracks
/// observable behavior rather than extent segmentation.
pub fn verify_full_materialization(src_path: &Path, dst_path: &Path) -> std::io::Result<()> {
    let src_bytes = fs::read(src_path)?;
    let dst_bytes = fs::read(dst_path)?;
    assert_eq!(src_bytes, dst_bytes, "destination bytes must match source bytes");
    Ok(())
}

/// Verifies partial materialization by checking every chunk against the source file
/// and requiring unwritten chunks to remain zeroed.
///
/// This helper is used when the sparse object has been only partially materialized.
/// Filled chunks must byte-match the source, while chunks that were not filled must
/// still read back as zeroes from the destination.
pub fn verify_partial_materialization(
    src_path: &Path,
    dst_path: &Path,
    filled_offsets: &HashSet<usize>,
    chunk_size: usize,
    len: usize,
) -> std::io::Result<()> {
    let mut src_file = fs::File::open(src_path)?;
    let mut dst_file = fs::File::open(dst_path)?;
    let dst_len = fs::metadata(dst_path)?.len() as usize;

    for offset in (0..len).step_by(chunk_size.max(1)) {
        let chunk_len = (len - offset).min(chunk_size.max(1));
        src_file.seek(SeekFrom::Start(offset as u64))?;

        let mut src_buf = vec![0u8; chunk_len];
        let mut dst_buf = vec![0u8; chunk_len];
        src_file.read_exact(&mut src_buf)?;

        if offset < dst_len {
            let readable = chunk_len.min(dst_len - offset);
            dst_file.seek(SeekFrom::Start(offset as u64))?;
            dst_file.read_exact(&mut dst_buf[..readable])?;
        }

        if filled_offsets.contains(&offset) {
            assert_eq!(dst_buf, src_buf, "filled chunk at offset {offset} must match the source");
        } else {
            assert!(dst_buf.iter().all(|byte| *byte == 0), "unfilled chunk at offset {offset} should stay zeroed");
        }
    }

    Ok(())
}
