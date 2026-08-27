<p align="center">
  <img width="500px" src="./docs/static/logo.png" alt="SparseIO">
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

<p align="center">
  <strong>Fetch only what you need. Store each unique chunk once.</strong>
</p>

SparseIO is infrastructure and an extensible Rust library for coordinating sparse,
out-of-order ranged fetches to materialize large-object
[content-addressable storage (CAS)](./docs/architecture/CAS.md).

Large objects are often consumed a few ranges at a time: a tensor from a model, a
row group from a dataset, several blocks from a backup, or a segment from a media
file. Fetching the entire object before serving the first useful byte wastes time,
bandwidth, and storage. SparseIO materializes an object incrementally instead. A
requested range is fetched from its upstream source, split into stable chunks, and
stored by content hash so future reads can reuse it.

<p align="center">
  <img
    width="600px"
    src="./docs/static/general-read.gif"
    alt="SparseIO animation showing a cache miss, prefetch, and cache hit as sparse chunks materialize."
  >
</p>

## Why SparseIO?

| Whole-object caching | SparseIO |
| --- | --- |
| Downloads every byte on the first miss | Fetches only the ranges callers request |
| Stores repeated data once per object | Deduplicates equal chunks by content hash |
| Can duplicate work during concurrent misses | Coalesces in-flight requests for the same chunk |
| Couples the cache to a source or runtime | Uses pluggable, executor-neutral backend traits |

This is especially useful for:

- AI/ML models and datasets where only selected tensors or shards are needed.
- Database backups, VM images, and archives explored without a full restore.
- Columnar data, logs, and scientific data read non-sequentially.
- Media and other large remote objects served through byte-range requests.

## How It Works

For each range read, SparseIO:

1. Normalizes the requested range into fixed-size chunks.
2. Looks up each chunk in the metadata index and local cache.
3. Fetches missing chunks from the registered upstream [`Reader`](./docs/architecture/API.md#reader).
4. Coalesces concurrent misses so only one upstream fetch does the work.
5. Hashes and writes new chunks into the CAS through the configured [`Writer`](./docs/architecture/API.md#writer).
6. Returns the requested bytes while the object becomes incrementally available.

Because chunks are addressed by their content, identical regions can be shared across
objects and versions. A fine-tuned model, incremental database backup, or revised disk
image only needs storage for the chunks that actually changed.

## Library and Infrastructure

SparseIO is designed to work at two levels:

- **Embedded library:** compose storage systems directly in Rust using small,
  object-safe [`Reader`](./docs/architecture/API.md#reader),
  [`Writer`](./docs/architecture/API.md#writer), and
  [`Metadata`](./docs/architecture/API.md#metadata) traits. The core remains independent
  of Tokio or any other specific async executor.
- **Deployable infrastructure:** expose sparse materialization to applications and
  non-Rust clients through service interfaces.

Planned infrastructure includes:

- A [Redis/RESP][redis-resp] interface for accessing SparseIO from existing clients and tooling.
- In-memory peers and trackers inspired by [Meta's Owl architecture][meta-owl]
  for high-fanout, peer-assisted chunk distribution. Peers cache and transfer chunks;
  trackers coordinate where peers fetch them and maintain a view of distribution
  state.

The backend contracts intentionally stay narrow:

| Component | Responsibility | Example implementations |
| --- | --- | --- |
| [`Reader`](./docs/architecture/API.md#reader) | Fetch byte ranges from an upstream object | HTTP, S3, Hugging Face, local files |
| [`Writer`](./docs/architecture/API.md#writer) | Store and retrieve content-addressed chunks | Local disk, object storage, distributed caches |
| [`Metadata`](./docs/architecture/API.md#metadata) | Track object coverage and chunk lifecycle | Redis, another key-value store, embedded state |
| [`ReaderRegistry`](./docs/architecture/OBJECTS.md#readerregistry) | Route canonical object paths to readers | Application-defined source schemes |

Bring the systems that fit your workload; SparseIO coordinates the read path, sparse
coverage, in-flight work, and CAS materialization.

## Design Goals

- **Sparse by default:** requesting one range never requires materializing the whole
  object.
- **Backend agnostic:** sources, chunk storage, and metadata are replaceable.
- **Runtime neutral:** the library remains usable from Tokio, smol, async-std, and
  other executors.
- **Safe under concurrency:** overlapping requests share work instead of multiplying
  upstream traffic.
- **Cache, not custody:** missing or expired cached chunks fall back to the source of
  truth.

## Project Status

SparseIO is under active development. The core traits and architecture are taking
shape, but the read path, backend integrations, service infrastructure, and public API
are not yet ready for production use. The Redis/RESP interface and Owl-inspired
in-memory peer and tracker implementations are planned work. Feedback from storage,
data infrastructure, and ML systems builders is welcome while these interfaces are
still evolving.

## Documentation

- [Architecture and design](./docs/index.md)
- [Testing](./docs/testing/index.md)
- [Trait validation](./docs/testing/VALIDATION.md)
- [Trait API](./docs/architecture/API.md)
- [Content-addressable storage](./docs/architecture/CAS.md)
- [Read flow](./docs/architecture/FLOW.md)
- [Library API documentation](https://docs.rs/sparseio)

[meta-owl]: https://engineering.fb.com/2022/07/14/data-infrastructure/owl-distributing-content-at-meta-scale/
[redis-resp]: https://redis.io/docs/latest/develop/reference/protocol-spec/
