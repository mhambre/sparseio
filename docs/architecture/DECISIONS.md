# Performance Decisions

SparseIO keeps the normal read path narrow because cache latency compounds across many
chunks.

## Singleflight

Asyncband coalesces concurrent work by URI hash, object generation, and normalized
chunk offset. One leader performs metadata, writer, or upstream work while followers
await the same result. The key includes the generation so invalidation never joins new
reads to stale work.

Independent chunks are polled concurrently up to the immutable per-instance limit.
This supplies bounded parallelism without installing an executor or creating
background workers.

## Buffering

Backend payloads use owned `Bytes`, allowing cheap clones and slices. A result within
one chunk is returned as a zero-copy slice. A result spanning chunks uses one
exact-capacity allocation and copies each requested segment once.

URI hashes and metadata prefixes are prepared when a viewer is created. SHA-256 output
is encoded directly to lowercase hexadecimal without formatting machinery. Metadata
mappings use a compact length-prefixed binary representation and do not require a
serialization dependency.

## Metadata and Disk

`MemoryMetadata` uses a read-write locked `BTreeMap`. Reads can proceed concurrently,
prefix scans begin at the requested bound, and compare-exchange holds one write lock
for its complete atomic operation.

`DiskWriter` writes to a unique temporary file and atomically publishes it. Reads
perform one blocking task and preallocate from file metadata. On Unix, all entry
operations are relative to a pinned directory handle and reject symbolic-link entries,
preventing path traversal and cache-root replacement from redirecting I/O.

## Lifecycle Cost

Permanent caching adds no expiration metadata work. Expiring mode refreshes one
content expiration on cache use. Global mapping scans and deletion decisions remain in
explicit garbage collection, outside ordinary reads. See [CAS](./CAS.md) for the
lifecycle protocol.

Divan benchmarks isolate component costs for regression review. A separate loopback
API harness applies controlled latency, jitter, bandwidth, and concurrency while
recording tail latency and observed upstream traffic. Neither result is a portable
latency promise.
