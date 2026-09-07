# Read Flow

`SparseIO::open` resolves the URI scheme and creates an object-bound `Viewer`. The
viewer caches the upstream length after its first successful lookup.

For `Viewer::read_at(offset, length)`, SparseIO:

1. Validates overflow and bounds, then truncates the requested end at EOF.
2. Loads the object's invalidation generation once.
3. Normalizes the window into fixed chunk offsets.
4. Polls independent chunk operations in order with the configured concurrency limit.
5. Coalesces equal URI, generation, and chunk requests through Asyncband singleflight.
6. Looks up `mapping:{uri_hash}:{offset}` and reads the mapped content hash from the
   writer when its generation is current.
7. Fetches an exact chunk from the upstream reader on a miss, hashes it with SHA-256,
   stores it, and publishes the mapping.
8. Slices or assembles the normalized chunks into the caller's exact result.

A single-chunk result is sliced from `Bytes` without copying. Multi-chunk results make
one exact-capacity allocation. A completed cold read awaits cache and metadata
publication, so the next read is immediately warm.

## Concurrency and Invalidation

Generation values are part of flight and mapping identities. Invalidation atomically
changes the generation before removing old mappings with compare-and-delete. A source
read checks the generation before and after publication. These checks prevent a read
that raced invalidation from making stale coverage visible.

Malformed mappings, wrong-length blobs, and missing writer keys are recoverable misses.
Cleanup uses compare-and-delete so it cannot remove a mapping another request replaced.

## Expiration

Expiring mode refreshes `expiry:{content_hash}` during materialization and cache hits.
`collect_garbage` explicitly acquires `gc:lock`, scans current mappings, refreshes live
hashes, and deletes expired unreferenced blobs. Renewable publication markers protect
slow writes, while deletion markers prevent publication during removal. Collection also
resumes stale unreferenced markers. SparseIO starts no autonomous background work.
