#![cfg(feature = "test-utils")]

use std::sync::Arc;

use bytes::Bytes;
use sparseio::Builder;
use sparseio::utils::{counting, fixture, oracle, tracing};

async fn build_io<R, W>(
    object_id: impl Into<String>,
    reader: R,
    writer: W,
    metadata: oracle::Metadata,
    chunk_size: usize,
) -> sparseio::SparseIO<R, W, oracle::Metadata>
where
    R: sparseio::Reader + Send + Sync + 'static,
    W: sparseio::Writer + Send + Sync + 'static,
{
    Builder::new()
        .object_id(object_id)
        .chunk_size(chunk_size)
        .metadata(metadata)
        .reader(reader)
        .writer(writer)
        .build()
        .await
        .expect("builder should succeed")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn builder_validation_rejects_missing_fields() {
    tracing::init();

    let reader = oracle::Reader::new(fixture::bytes(32));
    let writer = oracle::Writer::default();

    let missing_object_id = match Builder::<oracle::Reader, oracle::Writer, oracle::Metadata>::new()
        .metadata(oracle::Metadata::new())
        .reader(reader.clone())
        .writer(writer.clone())
        .build()
        .await
    {
        Ok(_) => panic!("missing object id should fail"),
        Err(err) => err,
    };
    assert_eq!(missing_object_id.kind(), std::io::ErrorKind::InvalidInput);

    let missing_reader = match Builder::<oracle::Reader, oracle::Writer, oracle::Metadata>::new()
        .object_id("test://missing-reader")
        .metadata(oracle::Metadata::new())
        .writer(writer.clone())
        .build()
        .await
    {
        Ok(_) => panic!("missing reader should fail"),
        Err(err) => err,
    };
    assert_eq!(missing_reader.kind(), std::io::ErrorKind::InvalidInput);

    let missing_writer = match Builder::<oracle::Reader, oracle::Writer, oracle::Metadata>::new()
        .object_id("test://missing-writer")
        .metadata(oracle::Metadata::new())
        .reader(reader.clone())
        .build()
        .await
    {
        Ok(_) => panic!("missing writer should fail"),
        Err(err) => err,
    };
    assert_eq!(missing_writer.kind(), std::io::ErrorKind::InvalidInput);

    let missing_metadata = match Builder::<oracle::Reader, oracle::Writer, oracle::Metadata>::new()
        .object_id("test://missing-metadata")
        .reader(reader)
        .writer(writer)
        .build()
        .await
    {
        Ok(_) => panic!("missing metadata should fail"),
        Err(err) => err,
    };
    assert_eq!(missing_metadata.kind(), std::io::ErrorKind::InvalidInput);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cache_transitions_from_uncached_to_cached_without_extra_upstream_reads() {
    tracing::init();

    let fixture = fixture::bytes(64);
    let reader = counting::Reader::new(oracle::Reader::new(fixture.clone()));
    let writer = counting::Writer::new(oracle::Writer::default());
    let io = Arc::new(build_io("test://cache-hit", reader.clone(), writer.clone(), oracle::Metadata::new(), 16).await);

    let first = io.read_chunk(0).await.expect("first read should succeed");
    let second = io.read_chunk(0).await.expect("second read should succeed");

    assert_eq!(first, second);
    assert_eq!(reader.read_count(), 1, "upstream reader should be used once");
    assert_eq!(writer.create_count(), 1, "payload should be materialized once");
    assert_eq!(writer.read_count(), 1, "cached re-read should come from the writer");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn same_offset_concurrency_dedupe_shares_one_upstream_fetch() {
    tracing::init();

    let fixture = fixture::bytes(128);
    let reader = counting::Reader::new(oracle::Reader::new(fixture.clone()))
        .with_read_delay(std::time::Duration::from_millis(10));
    let io = Arc::new(
        build_io("test://dedupe", reader.clone(), oracle::Writer::default(), oracle::Metadata::new(), 32).await,
    );

    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let io = io.clone();
            tokio::spawn(async move { io.read_chunk(0).await })
        })
        .collect();

    for task in tasks {
        let chunk = task.await.expect("task should join").expect("chunk read should succeed");
        assert_eq!(chunk, fixture.slice(0..32));
    }

    assert_eq!(reader.read_count(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_chunks_survive_one_object_clear_until_last_reference_is_removed() {
    tracing::init();

    let data = fixture::bytes(32);
    let metadata = oracle::Metadata::new();

    let first = Arc::new(
        build_io("test://first", oracle::Reader::new(data.clone()), oracle::Writer::default(), metadata.clone(), 32)
            .await,
    );
    let second = Arc::new(
        build_io("test://second", oracle::Reader::new(data.clone()), oracle::Writer::default(), metadata.clone(), 32)
            .await,
    );

    assert_eq!(first.read_chunk(0).await.expect("first read should work"), data);
    assert_eq!(second.read_chunk(0).await.expect("second read should work"), data);

    let mut first_viewer = first.viewer();
    first_viewer.clear_cache().await.expect("first clear should succeed");

    assert_eq!(
        second.read_chunk(0).await.expect("second object should still be readable"),
        data,
        "clearing one object should not remove shared payloads still needed by another object"
    );

    let mut second_viewer = second.viewer();
    second_viewer.clear_cache().await.expect("second clear should succeed");

    let reopened = Arc::new(
        build_io(
            "test://second",
            counting::Reader::new(oracle::Reader::new(data.clone())),
            oracle::Writer::default(),
            metadata,
            32,
        )
        .await,
    );
    assert_eq!(
        reopened.read_chunk(0).await.expect("reopened object should still read"),
        data,
        "after clearing both objects, a reopened read should refetch from upstream instead of relying on stale metadata"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reopened_object_reuses_persisted_chunk_size_and_length() {
    tracing::init();

    let fixture = fixture::bytes(64);
    let metadata = oracle::Metadata::new();
    let first = Arc::new(
        build_io(
            "test://reopen",
            oracle::Reader::new(fixture.clone()),
            oracle::Writer::default(),
            metadata.clone(),
            16,
        )
        .await,
    );
    assert_eq!(first.read_chunk(48).await.expect("tail should materialize"), fixture.slice(48..64));

    let reopened = build_io(
        "test://reopen",
        oracle::Reader::new(Bytes::from_static(b"this length should be ignored")),
        oracle::Writer::default(),
        metadata,
        32,
    )
    .await;

    assert_eq!(reopened.chunk_size(), 16);
    assert_eq!(reopened.len(), 64);
}
