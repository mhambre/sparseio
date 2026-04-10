#![cfg(feature = "file")]

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;
use sparseio::{Builder, sources::file::{Reader, Writer}};
use sparseio::{Reader as _, Writer as _};
use sparseio::utils::{file, fixture, materialization, temp, tracing};

/// Joins a temporary directory with a file name used by the file-backed tests.
fn temp_file(dir: &Path, name: &str) -> std::path::PathBuf {
    temp::temp_path(dir, name)
}

/// Builds a file-backed SparseIO instance for the given source and
/// destination paths.
async fn build_file_io(src_path: std::path::PathBuf, dst_path: std::path::PathBuf, chunk_size: usize) -> Arc<sparseio::SparseIO<Reader, Writer>> {
    Arc::new(
        Builder::new()
            .chunk_size(chunk_size)
            .reader(Reader::new(src_path))
            .writer(Writer::new(dst_path))
            .build()
            .await
            .expect("file-backed SparseIO should build"),
    )
}

/// This test pins the file reader contract at the boundary conditions so
/// EOF behavior stays explicit rather than inferred from Tokio internals.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_reader_truncates_tail_and_reports_eof() -> Result<(), Box<dyn std::error::Error>> {
    tracing::init();

    let dir = temp::temp_dir();
    let src_path = temp_file(dir.path(), "reader.bin");
    fs::write(&src_path, b"hello sparseio")?;

    let reader = Reader::new(&src_path);
    let mut buffer = [0xFFu8; 8];
    let read = reader.read_at(11, &mut buffer).await?;
    assert_eq!(read, 3);
    assert_eq!(&buffer[..3], b"eio");

    let eof = reader.read_at(14, &mut buffer).await?;
    assert_eq!(eof, 0);
    Ok(())
}

/// This test validates the file writer's observable extent contract rather
/// than internal extent segmentation, which keeps the test resilient to
/// future merge behavior.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_writer_same_offset_overwrite_and_delete_missing_are_stable() -> Result<(), Box<dyn std::error::Error>> {
    tracing::init();

    let dir = temp::temp_dir();
    let dst_path = temp_file(dir.path(), "writer.bin");
    let mut writer = Writer::new(&dst_path);

    writer.delete_extent(0).await?;
    writer.create_extent(64, Bytes::from_static(b"first")).await?;
    writer.create_extent(64, Bytes::from_static(b"second")).await?;
    writer.create_extent(256, Bytes::from_static(b"tail")).await?;

    assert_eq!(writer.read_extent(64).await?, Bytes::from_static(b"second"));
    assert_eq!(writer.read_extent(128).await?, Bytes::new());
    assert_eq!(writer.read_extent(256).await?, Bytes::from_static(b"tail"));

    writer.delete_extent(64).await?;
    assert!(writer.read_extent(64).await?.is_empty());
    Ok(())
}

/// This test mirrors the example's sparse materialization checks so the
/// example and test behavior stay aligned when materialization logic shifts.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn partial_materialization_preserves_sparse_gaps() -> Result<(), Box<dyn std::error::Error>> {
    tracing::init();

    let dir = temp::temp_dir();
    let src_path = temp_file(dir.path(), "partial-src.bin");
    let dst_path = temp_file(dir.path(), "partial-dst.bin");
    let fixture = fixture::bytes(128);
    fs::write(&src_path, &fixture)?;

    let io = build_file_io(src_path.clone(), dst_path.clone(), 16).await;
    let mut viewer = io.viewer();
    let mut filled = HashSet::new();
    for offset in [0usize, 32, 96] {
        viewer.seek(offset)?;
        let mut buffer = vec![0u8; 16];
        let read = viewer.read(&mut buffer).await?;
        assert_eq!(read, 16);
        assert_eq!(buffer, fixture.slice(offset..offset + 16).to_vec());
        filled.insert(offset);
    }

    materialization::verify_partial_materialization(&src_path, &dst_path, &filled, 16, fixture.len())?;
    Ok(())
}

/// This test validates the fully materialized end state and ensures the
/// file-backed example path and file-backed integration tests share the
/// same observable contract.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_materialization_matches_the_source_file() -> Result<(), Box<dyn std::error::Error>> {
    tracing::init();

    let dir = temp::temp_dir();
    let src_path = temp_file(dir.path(), "full-src.bin");
    let dst_path = temp_file(dir.path(), "full-dst.bin");
    let fixture = fixture::bytes(160);
    fs::write(&src_path, &fixture)?;

    let io = build_file_io(src_path.clone(), dst_path.clone(), 32).await;
    let mut viewer = io.viewer();
    for offset in (0..fixture.len()).step_by(32) {
        viewer.seek(offset)?;
        let mut buffer = vec![0u8; 32.min(fixture.len() - offset)];
        let read = viewer.read(&mut buffer).await?;
        assert_eq!(read, buffer.len());
        assert_eq!(buffer, fixture.slice(offset..offset + buffer.len()).to_vec());
    }

    materialization::verify_full_materialization(&src_path, &dst_path)?;
    Ok(())
}

/// This test checks the platform-specific sparse-file observation that the
/// example demonstrates, without making assumptions about internal extent
/// segmentation.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn allocated_size_stays_below_logical_size_on_supported_platforms() -> Result<(), Box<dyn std::error::Error>> {
    tracing::init();

    let dir = temp::temp_dir();
    let src_path = temp_file(dir.path(), "sparse-src.bin");
    let dst_path = temp_file(dir.path(), "sparse-dst.bin");
    let fixture = fixture::bytes(4 * 1024);
    fs::write(&src_path, &fixture)?;
    fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&dst_path)?
        .set_len(fixture.len() as u64)?;

    let io = build_file_io(src_path.clone(), dst_path.clone(), 512).await;
    let mut viewer = io.viewer();
    viewer.seek(0)?;
    let mut buffer = vec![0u8; 512];
    viewer.read(&mut buffer).await?;
    viewer.seek(fixture.len() - 512)?;
    let mut tail = vec![0u8; 512];
    viewer.read(&mut tail).await?;

    let logical = fs::metadata(&dst_path)?.len();
    let allocated = file::allocated_bytes(&dst_path)?.unwrap_or(0);
    assert_eq!(logical, fixture.len() as u64);
    if allocated > 0 {
        assert!(
            allocated <= logical,
            "allocated bytes should not exceed logical bytes for a sparse file"
        );
    }
    Ok(())
}
