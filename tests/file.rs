#![cfg(all(feature = "impl-file", feature = "test-utils"))]

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use sparseio::sources::file::{Reader, Writer};
use sparseio::utils::{counting, fixture, temp, tracing};
use sparseio::{Builder, Reader as _};

fn temp_path(dir: &Path, name: &str) -> std::path::PathBuf {
    temp::temp_path(dir, name)
}

async fn build_file_io(
    src_path: std::path::PathBuf,
    cache_dir: std::path::PathBuf,
    metadata: sparseio::utils::oracle::Metadata,
    chunk_size: usize,
) -> Arc<sparseio::SparseIO<Reader, Writer, sparseio::utils::oracle::Metadata>> {
    Arc::new(
        Builder::new()
            .chunk_size(chunk_size)
            .object_id(format!("file://{}", src_path.display()))
            .metadata(metadata)
            .reader(Reader::new(src_path))
            .writer(Writer::new(cache_dir))
            .build()
            .await
            .expect("file-backed SparseIO should build"),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_reader_truncates_tail_and_reports_eof() -> Result<(), Box<dyn std::error::Error>> {
    tracing::init();

    let dir = temp::temp_dir();
    let src_path = temp_path(dir.path(), "reader.bin");
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_file_reader_reuses_file_cache_metadata() -> Result<(), Box<dyn std::error::Error>> {
    tracing::init();

    let dir = temp::temp_dir();
    let src_path = temp_path(dir.path(), "reload-src.bin");
    let cache_dir = temp_path(dir.path(), "reload-cache");
    let fixture = fixture::bytes(64);
    let metadata = sparseio::utils::oracle::Metadata::new();
    fs::write(&src_path, &fixture)?;

    let first = build_file_io(src_path.clone(), cache_dir.clone(), metadata.clone(), 16).await;
    assert_eq!(first.read_chunk(0).await?, fixture.slice(0..16));
    assert_eq!(first.read_chunk(48).await?, fixture.slice(48..64));

    let second = build_file_io(src_path, cache_dir, metadata, 32).await;
    assert_eq!(second.chunk_size(), 16);
    assert_eq!(second.read_chunk(0).await?, fixture.slice(0..16));
    assert_eq!(second.read_chunk(48).await?, fixture.slice(48..64));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_writer_deduplicates_identical_chunks_across_identities() -> Result<(), Box<dyn std::error::Error>> {
    tracing::init();

    let dir = temp::temp_dir();
    let cache_dir = temp_path(dir.path(), "shared-cache");
    let src_a = temp_path(dir.path(), "source-a.bin");
    let src_b = temp_path(dir.path(), "source-b.bin");
    let data = fixture::bytes(32);
    let metadata = sparseio::utils::oracle::Metadata::new();
    fs::write(&src_a, &data)?;
    fs::write(&src_b, &data)?;

    let first = build_file_io(src_a.clone(), cache_dir.clone(), metadata.clone(), 32).await;
    assert_eq!(first.read_chunk(0).await?, data.slice(0..32));

    let second = build_file_io(src_b.clone(), cache_dir.clone(), metadata, 32).await;
    assert_eq!(second.read_chunk(0).await?, data.slice(0..32));

    let names = fs::read_dir(&cache_dir)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(names.len(), 1);

    let mut first_viewer = first.viewer();
    first_viewer.clear_cache().await?;

    let remaining_after_first_clear = fs::read_dir(&cache_dir)?.count();
    assert_eq!(
        remaining_after_first_clear, 1,
        "shared chunk should remain while second identity still references it"
    );

    let mut second_viewer = second.viewer();
    second_viewer.clear_cache().await?;
    let remaining_after_second_clear = fs::read_dir(&cache_dir)?.count();
    assert_eq!(remaining_after_second_clear, 0, "chunk should be deleted once the final reference is cleared");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn file_sparseio_repairs_missing_chunk_file_on_reopen() -> Result<(), Box<dyn std::error::Error>> {
    tracing::init();

    let dir = temp::temp_dir();
    let src_path = temp_path(dir.path(), "repair-src.bin");
    let cache_dir = temp_path(dir.path(), "repair-cache");
    let fixture = fixture::bytes(64);
    let metadata = sparseio::utils::oracle::Metadata::new();
    fs::write(&src_path, &fixture)?;

    let first = build_file_io(src_path.clone(), cache_dir.clone(), metadata.clone(), 16).await;
    assert_eq!(first.read_chunk(0).await?, fixture.slice(0..16));

    let chunk_path = fs::read_dir(&cache_dir)?
        .next()
        .expect("cache directory should contain one chunk")?
        .path();
    fs::remove_file(&chunk_path)?;

    let reader = counting::Reader::new(Reader::new(src_path.clone()));
    let reopened = Arc::new(
        Builder::new()
            .object_id(format!("file://{}", src_path.display()))
            .chunk_size(32)
            .metadata(metadata)
            .reader(reader.clone())
            .writer(Writer::new(cache_dir.clone()))
            .build()
            .await?,
    );

    assert_eq!(reopened.chunk_size(), 16);
    assert_eq!(reopened.read_chunk(0).await?, fixture.slice(0..16));
    assert_eq!(reader.read_count(), 1, "missing chunk file should trigger one upstream refetch");
    assert_eq!(fs::read_dir(&cache_dir)?.count(), 1, "repaired chunk should be recreated on disk");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn file_sparseio_dedupes_same_offset_concurrency_with_real_backend() -> Result<(), Box<dyn std::error::Error>> {
    tracing::init();

    let dir = temp::temp_dir();
    let src_path = temp_path(dir.path(), "concurrency-src.bin");
    let cache_dir = temp_path(dir.path(), "concurrency-cache");
    let fixture = fixture::bytes(96);
    fs::write(&src_path, &fixture)?;

    let reader = counting::Reader::new(Reader::new(src_path)).with_read_delay(Duration::from_millis(10));
    let writer = counting::Writer::new(Writer::new(cache_dir.clone()));
    let io = Arc::new(
        Builder::new()
            .chunk_size(16)
            .object_id("file://concurrency-src")
            .metadata(sparseio::utils::oracle::Metadata::new())
            .reader(reader.clone())
            .writer(writer.clone())
            .build()
            .await?,
    );

    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let io = io.clone();
            tokio::spawn(async move { io.read_chunk(0).await })
        })
        .collect();
    for task in tasks {
        assert_eq!(task.await??, fixture.slice(0..16));
    }

    assert_eq!(reader.read_count(), 1, "same-offset file reads should dedupe upstream work");
    assert_eq!(writer.create_count(), 1, "only one chunk should be materialized into the file cache");
    assert_eq!(fs::read_dir(&cache_dir)?.count(), 1, "only one on-disk chunk file should be created");
    Ok(())
}
