#![cfg(feature = "utils")]

use std::sync::Arc;

use bytes::Bytes;
use futures::StreamExt;
use sparseio::Builder;
use sparseio::utils::{counting, fixture, flaky, oracle, tracing};

/// Builds a SparseIO instance for the integration tests using the supplied
/// reader, writer, and chunk size.
///
/// Keeping this helper local avoids repeating builder setup across scenarios
/// while still making the configured contract explicit in one place.
async fn build_io<R, W>(reader: R, writer: W, chunk_size: usize) -> sparseio::SparseIO<R, W>
where
    R: sparseio::Reader + Send + Sync + 'static,
    W: sparseio::Writer + Send + Sync + 'static,
{
    Builder::new()
        .chunk_size(chunk_size)
        .reader(reader)
        .writer(writer)
        .build()
        .await
        .expect("builder should succeed")
}

/// This test fixes the contract around builder validation so downstream
/// harnesses can rely on explicit failures instead of panics or silent
/// defaults when required inputs are missing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn builder_validation_rejects_missing_fields_and_zero_chunk_size() {
    tracing::init();

    let reader = oracle::Reader::new(fixture::bytes(32));
    let writer = oracle::Writer::default();

    let missing_reader = match Builder::<oracle::Reader, oracle::Writer>::new()
        .writer(writer.clone())
        .build()
        .await
    {
        Ok(_) => panic!("missing reader should fail"),
        Err(err) => err,
    };
    assert_eq!(missing_reader.kind(), std::io::ErrorKind::InvalidInput);

    let missing_writer = match Builder::<oracle::Reader, oracle::Writer>::new()
        .reader(reader.clone())
        .build()
        .await
    {
        Ok(_) => panic!("missing writer should fail"),
        Err(err) => err,
    };
    assert_eq!(missing_writer.kind(), std::io::ErrorKind::InvalidInput);

    let zero_chunk = match Builder::new().chunk_size(0).reader(reader).writer(writer).build().await {
        Ok(_) => panic!("zero chunk size should fail"),
        Err(err) => err,
    };
    assert_eq!(zero_chunk.kind(), std::io::ErrorKind::InvalidInput);
}

/// This test exercises the common read path at chunk-aligned and
/// unaligned offsets so SparseIO cannot regress to only handling exact
/// chunk boundaries.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aligned_and_unaligned_viewer_reads_match_the_fixture() {
    tracing::init();

    let fixture = fixture::bytes(96);
    let io = Arc::new(
        build_io(
            counting::Reader::new(oracle::Reader::new(fixture.clone())),
            counting::Writer::new(oracle::Writer::default()),
            16,
        )
        .await,
    );

    let mut viewer = io.viewer();
    viewer.seek(5).expect("seek should succeed");

    let mut buf = vec![0u8; 37];
    let read = viewer.read(&mut buf).await.expect("read should succeed");
    assert_eq!(read, 37);
    assert_eq!(buf, fixture.slice(5..42).to_vec());

    viewer.seek(16).expect("aligned seek should succeed");
    let mut aligned = vec![0u8; 16];
    let read = viewer.read(&mut aligned).await.expect("aligned read should succeed");
    assert_eq!(read, 16);
    assert_eq!(aligned, fixture.slice(16..32).to_vec());
}

/// This test ensures the first miss materializes an extent and the second
/// read is served from the cache layer rather than re-fetching upstream.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cache_transitions_from_uncached_to_cached_without_extra_upstream_reads() {
    tracing::init();

    let fixture = fixture::bytes(64);
    let reader = counting::Reader::new(oracle::Reader::new(fixture.clone()));
    let writer = counting::Writer::new(oracle::Writer::default());
    let io = Arc::new(build_io(reader.clone(), writer.clone(), 16).await);

    let first = io.read_chunk(0).await.expect("first read should succeed");
    let second = io.read_chunk(0).await.expect("second read should succeed");

    assert_eq!(first, second);
    assert_eq!(reader.read_count(), 1, "upstream reader should be used once");
    assert_eq!(writer.create_count(), 1, "the extent should be materialized once");
    assert_eq!(writer.read_count(), 1, "the cached re-read should come from the writer");
}

/// This test documents the intended `read_chunk` contract: callers may pass an
/// unaligned offset and receive the full chunk that contains that logical byte.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_chunk_normalizes_to_the_containing_chunk() {
    tracing::init();

    let fixture = fixture::bytes(64);
    let io = Arc::new(
        build_io(
            counting::Reader::new(oracle::Reader::new(fixture.clone())),
            counting::Writer::new(oracle::Writer::default()),
            16,
        )
        .await,
    );

    let chunk = io.read_chunk(17).await.expect("unaligned chunk read should succeed");
    assert_eq!(chunk, fixture.slice(16..32));
}

/// This test protects the in-flight dedupe path so concurrent callers at
/// the same offset do not multiply upstream work.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn same_offset_concurrency_dedupe_shares_one_upstream_fetch() {
    tracing::init();

    let fixture = fixture::bytes(128);
    let reader = counting::Reader::new(oracle::Reader::new(fixture.clone()));
    let writer = counting::Writer::new(oracle::Writer::default());
    let io = Arc::new(build_io(reader.clone(), writer, 32).await);

    let handles: Vec<_> = (0..12)
        .map(|_| {
            let io = io.clone();
            tokio::spawn(async move { io.read_chunk(0).await })
        })
        .collect();

    for handle in handles {
        let chunk = handle.await.expect("task should join").expect("chunk read should succeed");
        assert_eq!(chunk, fixture.slice(0..32));
    }

    assert_eq!(reader.read_count(), 1, "same-offset concurrency should dedupe");
}

/// This test verifies that dedupe does not leak across independent chunks
/// and that concurrent reads at different offsets still return the right
/// bytes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn different_offset_concurrency_returns_correct_chunks() {
    tracing::init();

    let fixture = fixture::bytes(192);
    let reader = counting::Reader::new(oracle::Reader::new(fixture.clone()));
    let writer = counting::Writer::new(oracle::Writer::default());
    let io = Arc::new(build_io(reader.clone(), writer, 32).await);

    let offsets = [0usize, 32, 64, 96];
    let handles: Vec<_> = offsets
        .into_iter()
        .map(|offset| {
            let io = io.clone();
            tokio::spawn(async move { (offset, io.read_chunk(offset).await) })
        })
        .collect();

    for handle in handles {
        let (offset, chunk) = handle.await.expect("task should join");
        let chunk = chunk.expect("chunk read should succeed");
        let expected = fixture.slice(offset..offset + 32);
        assert_eq!(chunk, expected, "chunk at offset {offset} should match the fixture");
    }

    assert_eq!(reader.read_count(), offsets.len(), "different offsets should fetch independently");
}

/// This test checks the stream path separately from buffered reads so the
/// byte stream remains a parity-preserving view over the same data.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bytestream_matches_the_fixture_payload() {
    tracing::init();

    let fixture = fixture::bytes(80);
    let io = Arc::new(
        build_io(
            counting::Reader::new(oracle::Reader::new(fixture.clone())),
            counting::Writer::new(oracle::Writer::default()),
            16,
        )
        .await,
    );

    let mut viewer = io.viewer();
    viewer.seek(7).expect("seek should succeed");
    let mut stream = viewer.to_bytestream().await;
    let mut collected = Vec::new();
    while let Some(chunk) = stream.next().await {
        collected.extend_from_slice(&chunk.expect("stream chunk should succeed"));
    }

    assert_eq!(Bytes::from(collected), fixture.slice(7..));
}

/// This test ensures a failing upstream read does not poison the in-flight
/// map, otherwise a transient reader error could permanently wedge the
/// chunk.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transient_reader_failures_cleanup_flights_and_allow_retry() {
    tracing::init();

    let fixture = fixture::bytes(64);
    let reader = counting::Reader::new(flaky::Reader::fail_once_at(fixture.clone(), [0]));
    let writer = counting::Writer::new(oracle::Writer::default());
    let io = Arc::new(build_io(reader.clone(), writer, 16).await);

    assert!(io.read_chunk(0).await.is_err(), "first transient failure should surface");
    let chunk = io.read_chunk(0).await.expect("retry should succeed");
    assert_eq!(chunk, fixture.slice(0..16));
    assert_eq!(reader.read_count(), 2, "retry should re-enter the upstream reader after cleanup");
}

/// This test exercises the writer failure path so a failed materialization
/// can be retried instead of leaving a stale flight behind.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transient_writer_failures_cleanup_flights_and_allow_retry() {
    tracing::init();

    let fixture = fixture::bytes(64);
    let reader = counting::Reader::new(oracle::Reader::new(fixture.clone()));
    let writer = flaky::Writer::fail_once_at([0]);
    let io = Arc::new(build_io(reader.clone(), writer, 16).await);

    assert!(io.read_chunk(0).await.is_err(), "first transient writer failure should surface");
    let chunk = io.read_chunk(0).await.expect("retry should succeed");
    assert_eq!(chunk, fixture.slice(0..16));
    assert_eq!(reader.read_count(), 2, "writer failure retry should refetch after cleanup");
}
