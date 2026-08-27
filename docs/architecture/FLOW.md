# Flow

This document describes how SparseIO processes a read request.

Constructing a [`SparseIO`](./OBJECTS.md#sparseio) instance requires a
[`Metadata`](./API.md#metadata) implementation, a [`Writer`](./API.md#writer)
implementation, and a [`ReaderRegistry`](./OBJECTS.md#readerregistry) loaded with the
[`Reader`](./API.md#reader) implementations the consumer wants to use. The
[`Builder`](./OBJECTS.md#sparseio) collects these dependencies and applies defaults.

Users call [`SparseIO::open`](./OBJECTS.md#viewer) with a canonical path to obtain a
[`Viewer`](./OBJECTS.md#viewer), for example
`instance.open("service://path/to/object")`. The primary operation is
`Viewer::read_at`, which supports both direct range reads and chunked byte streams.

When a user calls `Viewer::read_at` with an offset and length, SparseIO normalizes the
range to chunk boundaries. For example, with a 4 KiB chunk size, an offset of 5 KiB
starts in the chunk at 4 KiB. The metadata key for that chunk is
`metadata:{uri_hash}:{normalized_offset}`. SparseIO checks that key for a mapping and,
when one exists, asks the `Writer` to read the mapped cache chunk. It repeats this for
every chunk intersecting the requested range, capped at the end of the object.

## Read Path

On a cache miss, the `ReaderRegistry` supplies the `Reader` selected when the `Viewer`
was created. SparseIO fetches the missing chunk with `Reader::read_at`.

An in-memory flighting system prevents duplicate requests for the same chunk. It stores
a shared future for each chunk in progress so concurrent requests can await the same
work without repeating metadata or upstream reads. See the
[shared future store](./DECISIONS.md#shared-future-store) for details.

Once the parent reader receives the source bytes, SparseIO can return the requested
data while calculating the chunk hash and writing the chunk through the `Writer` under
`cache:{hash}`. It then maps `metadata:{uri_hash}:{normalized_offset}` to that cache key
for future reads.

SparseIO also writes the chunk expiry key `chunk:{hash}:expires_at`. Inserting or
reading a cached chunk refreshes its value to `now + cache_lifetime`. Garbage
collection can scan metadata mappings, refresh expiry for referenced chunks, and
delete expired chunks. If a mapping points to a missing chunk, SparseIO treats it as a
cache miss and fetches the data from upstream again. The
[CAS lifecycle](./CAS.md#deletion) describes this recovery model in more detail.
