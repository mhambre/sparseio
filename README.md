<p align="center">
  <img width="500px" src="./docs/static/logo.png">
</p>

<p align="center">
  <a href="https://github.com/mhambre/sparseio/actions/workflows/ci-ubuntu.yml">
    <img alt="Ubuntu CI workflow" src="https://github.com/mhambre/sparseio/actions/workflows/ci-ubuntu.yml/badge.svg"/>
  </a>
  <a href="https://github.com/mhambre/sparseio/actions/workflows/ci-macos.yml">
    <img alt="macOS CI workflow" src="https://github.com/mhambre/sparseio/actions/workflows/ci-macos.yml/badge.svg"/>
  </a>
  <a href="https://github.com/mhambre/sparseio/actions/workflows/github-code-scanning/codeql">
    <img src="https://github.com/mhambre/sparseio/actions/workflows/github-code-scanning/codeql/badge.svg" alt="CodeQL">
  </a>
  <a href="https://crates.io/crates/sparseio">
    <img src="https://img.shields.io/crates/v/sparseio.svg" alt="Crates.io">
  </a>
  <a href="https://docs.rs/sparseio">
    <img alt="docs.rs" src="https://img.shields.io/docsrs/sparseio">
  </a>
</p>

SparseIO is a Rust library for sparse, out-of-order materialization of large byte objects.

Instead of eagerly copying an entire object from source to destination, SparseIO allows you to fetch only the chunks you ask for. It tracks what is already present for efficient caching, and deduplicates concurrent reads for the same chunk.

<p align="center">
<img width="600px" src="./docs/static/general-read.gif" alt="SparseIO animation showing a cache miss, prefetch, and cache hit as sparse chunks materialize.">
</p>

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

## Current Sample Backends

- `impl-file`: file-backed `Reader`/`Writer` implementations.
- `impl-opendal`: OpenDAL-backed Reader integration.

- `metadata-memory`: in-memory metadata storage for single-process use.


## Quickstart

Run the local-file to file-cache example:

```bash
cargo run --example file_to_file --features impl-file,metadata-memory -- \
  --src target/manual/file-to-file-src.bin \
  --dst target/manual/file-to-file-cache \
  --source-len 8388608 \
  --chunk-size 262144
```

The example generates a local source file, reads a few chunk-aligned offsets into a file-cache directory, and then reopens the cache to show that metadata restores the original chunk size without having to cache the entire object.

See: `examples/file_to_file.rs` and `examples/file_to_file.md`.

## Minimal API Shape

```rust
use std::sync::Arc;
use opendal::Operator;

async fn demo() -> std::io::Result<()> {
  // HTTP File -> File Cache Directory
  let operator = Operator::from_uri(operator, "https://stuff.mit.edu");
  let reader = sparseio::sources::opendal::Reader::new(
    "afs/sipb/contrib/pi/pi-billion.txt",
  );
  let writer = sparseio::sources::file::Writer::new("pi-cache");
  let metadata = sparseio::metadata::memory::MemoryMetadata::new("pi-cache.metadata.bin", 1)?;

  let io = sparseio::Builder::new()
      .chunk_size(1 * 1024)
      .metadata(metadata)
      .reader(reader)
      .writer(writer)
      .build()
      .await?;

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

## Documentation

- [Usage Documentation](https://crates.io/crates/sparseio): Guides on how to use in your application.
- [Library Documentation](https://docs.rs/sparseio): Traits, methods, etc.
- [Development, Design, Architecture](./docs/index.md): Design decisions, diagrams, etc.
