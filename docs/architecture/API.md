# Trait API

Backend traits are object-safe, asynchronous, `Send + Sync`, and return
`sparseio::Result`. Payloads cross trait boundaries as owned [`Bytes`][bytes] values so
large buffers can move without mandatory copies. SparseIO does not install an executor
or start background work.

## `Reader`

`Reader` represents one immutable upstream object:

- `len()` returns its byte length.
- `read_at(offset, length)` returns exactly the requested range.

SparseIO normalizes calls to configured chunk boundaries and never asks a reader for a
range beyond the length it reported. Implementations should use native range operations;
downloading a complete remote object defeats sparse materialization.

## `ReaderFactory`

`ReaderFactory::create(uri)` receives the complete canonical object URI and returns a
shared object-bound reader. `ReaderRegistry` stores factories instead of constructed
readers, preserving trait-object safety and per-object configuration.

## `Writer`

`Writer` is a loose content-addressed cache:

- `write(key, value)` stores exact bytes under a key.
- `read(key)` returns `None` when the key is absent.
- `delete(key)` idempotently removes cached bytes.

SparseIO publishes a metadata mapping only after `write` succeeds. Writer data is never
treated as the source of truth.

## `Metadata`

`Metadata` stores coverage, object generations, expirations, and GC leases:

- `get`, `set`, and `delete` provide basic key-value operations.
- `scan_prefix` returns matching entries in backend-defined order.
- `compare_exchange` atomically replaces or deletes a value only when it equals the
  supplied expected value.

`compare_exchange` must distinguish an absent key from an empty byte value. It is the
consistency boundary used for invalidation cleanup and garbage-collection leases.
Distributed implementations remain responsible for their own connection, durability,
and retry behavior.

## Errors

Kinds and retryability are orthogonal. `ErrorKind` describes what failed while
`Retryability` indicates whether an identical later attempt may succeed. Backend errors
may preserve a source and operation label. SparseIO does not automatically retry in the
initial implementation.

## Reference Backends

- `metadata::MemoryMetadata` is enabled by `memory-metadata` and uses an `RwLock<BTreeMap>`.
- `writer::DiskWriter` is enabled by `disk-writer` and stores one safe filename per key.
- `reader::OpenDalReader` is enabled by `opendal-reader`; service features are selected
  individually with `opendal-reader-*` flags. SparseIO exposes only services that apply
  bounded reads at the storage or protocol boundary, plus zero-copy memory reads. OpenDAL
  adapters that fetch a complete value before slicing it are intentionally omitted.

OpenDAL does not expose a distinct bounded-read capability flag. URI-created readers are
therefore limited by an audited feature allowlist, while callers supplying a prebuilt
operator are responsible for preserving the same property.

## Backend Validation

The `validators` feature exposes executor-neutral `ReaderValidator`, `WriterValidator`,
and `MetadataValidator` workloads. Instrumented reference backends live behind the
`testing` feature. See [Trait Validation](../testing/VALIDATION.md).

[bytes]: https://docs.rs/bytes/latest/bytes/struct.Bytes.html
