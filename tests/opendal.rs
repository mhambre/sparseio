#![cfg(all(feature = "impl-opendal", feature = "test-utils"))]

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use sparseio::sources::opendal::Reader;
use sparseio::utils::{counting, oracle};
use sparseio::{Builder, Reader as _};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn opendal_reader_supports_ranged_reads_and_eof() -> Result<(), Box<dyn std::error::Error>> {
    let op = opendal::Operator::new(opendal::services::Memory::default())?.finish();
    op.write("object.bin", Bytes::from_static(b"hello sparseio")).await?;

    let reader = Reader::new(op, "object.bin").await?;
    assert_eq!(reader.len().await?, 14);

    let mut range = [0xFFu8; 8];
    let read = reader.read_at(6, &mut range).await?;
    assert_eq!(read, 8);
    assert_eq!(&range, b"sparseio");

    let mut tail = [0xAAu8; 8];
    let read = reader.read_at(11, &mut tail).await?;
    assert_eq!(read, 3);
    assert_eq!(&tail[..3], b"eio");
    assert_eq!(&tail[3..], &[0xAA; 5]);

    let eof = reader.read_at(14, &mut tail).await?;
    assert_eq!(eof, 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn opendal_reader_normalizes_paths() -> Result<(), Box<dyn std::error::Error>> {
    let op = opendal::Operator::new(opendal::services::Memory::default())?.finish();
    op.write("nested/object.bin", Bytes::from_static(b"normalize me")).await?;

    let reader = Reader::new(op, "./nested//object.bin").await?;
    assert_eq!(reader.path(), "nested/object.bin");

    let mut empty = [];
    assert_eq!(reader.read_at(0, &mut empty).await?, 0, "zero-length reads should be a no-op");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn opendal_sparseio_dedupes_same_offset_reads_and_reuses_cache() -> Result<(), Box<dyn std::error::Error>> {
    let op = opendal::Operator::new(opendal::services::Memory::default())?.finish();
    let fixture: Vec<u8> = (0..96).map(|index| index as u8).collect();
    op.write("object.bin", Bytes::from(fixture.clone())).await?;

    let reader = counting::Reader::new(Reader::new(op, "object.bin").await?).with_read_delay(Duration::from_millis(10));
    let writer = counting::Writer::new(oracle::Writer::default());
    let io = Arc::new(
        Builder::new()
            .object_id("memory://nested/object.bin")
            .chunk_size(16)
            .metadata(oracle::Metadata::new())
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
        assert_eq!(task.await??, Bytes::from(fixture[0..16].to_vec()));
    }

    assert_eq!(reader.read_count(), 1, "same-offset OpenDAL reads should dedupe upstream work");
    assert_eq!(writer.create_count(), 1, "only one extent should be materialized after the miss");

    let before_reads = reader.read_count();
    assert_eq!(io.read_chunk(0).await?, Bytes::from(fixture[0..16].to_vec()));
    assert_eq!(reader.read_count(), before_reads, "cached OpenDAL chunk should avoid another upstream read");
    assert!(writer.read_count() >= 1, "cached replay should read from the writer path");
    Ok(())
}
