# SparseIO

SparseIO is a Rust library for sparse, out-of-order materialization of large byte objects.

Instead of eagerly copying an entire object from source to destination, SparseIO allows you to fetch only the chunks you ask for. It tracks what is already present for efficient caching, and deduplicates concurrent reads for the same chunk.

## Core Premise

Certain large data objects, such as multimedia files, system logs, columnar storage files used in AI and ML workloads, and archival records, are often accessed non-sequentially. In these scenarios, applications typically retrieve only specific byte ranges rather than reading the entire object. Loading all bytes upfront results in unnecessary I/O, increased latency, and inefficient bandwidth utilization. Selective or partial reads improve performance by reducing data transfer, accelerating processing, and optimizing resource consumption.

SparseIO models this as:

1. A `Reader` that can fetch bytes from an upstream source at an offset.
2. A `Writer` that stores data extents in a local/closer destination representing the object sparsely.
3. A coordinator (`SparseIO`) that:
   - checks existing coverage for existing cache,
   - deduplicates in-flight fetches so concurrent callers do not duplicate work,
   - manages coverage metadata and cache.

## What You Get

- On-demand chunk materialization.
- Coverage-aware reads from an extent store.
- In-flight deduplication for concurrent requests.
- Pluggable backends via `Reader` and `Writer` traits.
- Optional source implementations in `sources` (feature-gated).

## Current Feature Flags

- `file`: file-backed `Reader`/`Writer` implementations.
- `http`: reqwesr-backed HTTP range-based `Reader` implementation.

## Quickstart

Run the file-to-file sparse example:

```bash
cargo run --example file_to_file --features file -- \
  --src target/manual/file-to-file-src.bin \
  --dst target/manual/file-to-file-dst.bin \
  --source-len 8388608 \
  --chunk-size 262144 \
  --fill-percent 35
```

The example intentionally materializes randomized chunk offsets first, then verifies:

- full fill => destination matches source byte-for-byte,
- partial fill => written chunks match source and unwritten regions remain zeroed.

See: `examples/file_to_file.rs` and `examples/file_to_file.md`.

## Minimal API Shape

```rust
use std::sync::Arc;
use sparseio::Builder;

async fn demo() -> std::io::Result<()> {
  // HTTP File -> Sparse Local File
  let reader = sparseio::sources::http::Reader::new("https://stuff.mit.edu/afs/sipb/contrib/pi/pi-billion.txt");
  let writer = sparseio::sources::file::Writer::new("pi.txt");

  let io = Arc::new(
    Builder::new()
      .chunk_size(1 * 1024)
      .reader(reader)
      .writer(writer)
      .build()
      .await?
  );

  // Get a viewer into the Sparse store
  let mut viewer = io.viewer();
  viewer.seek(1_000_002)?; // 2 to account for '3.'

  let mut buffer_1: [u8; 1] = [0]; // Digit 1,000,000
  let mut buffer_2: [u8; 1] = [0]; // Digit 1,000,001

  // Gets 1 KiB chunk containing digit from Webserver and fills buffer with chunk
  viewer.read(&mut buffer_1).await?;
  // Cached 1 KiB chunk means this is cached locally and much faster
  viewer.read(&mut buffer_2).await?;

  Ok(())
}
```
