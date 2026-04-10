#![cfg(feature = "debug")]

use sparseio::debug::{ReaderHarness, ReaderHarnessConfig, WriterHarness, WriterHarnessConfig};
use sparseio::utils::{fixture, oracle, tracing};

/// This test proves the public reader harness can validate a correct
/// reader against both direct byte-for-byte reads and SparseIO-backed
/// caching behavior without any local test-only hooks.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reader_harness_validates_oracle_reader_end_to_end() {
    tracing::init();

    let fixture = fixture::bytes(96);
    let harness = ReaderHarness::new(
        oracle::Reader::new(fixture.clone()),
        ReaderHarnessConfig {
            chunk_size: 16,
            fixture,
        },
    );

    harness.validate().await.expect("oracle reader should satisfy the harness");
}

/// This test proves the public writer harness can validate a correct
/// writer implementation without sharing state between the direct extent
/// tests and the SparseIO integration phase.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writer_harness_validates_oracle_writer_end_to_end() {
    tracing::init();

    let fixture = fixture::bytes(96);
    let harness = WriterHarness::new(
        || oracle::Writer::default(),
        WriterHarnessConfig {
            chunk_size: 16,
            fixture,
        },
    );

    harness.validate().await.expect("oracle writer should satisfy the harness");
}
