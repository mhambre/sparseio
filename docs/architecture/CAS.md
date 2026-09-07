# Content-Addressable Cache

SparseIO stores each materialized chunk under the lowercase hexadecimal SHA-256
digest of its bytes. Object metadata maps a canonical URI hash and normalized chunk
offset to that content key. Equal chunks therefore share one writer entry even when
they belong to different objects.

## Publication

A cold read follows this order:

1. Fetch the exact normalized range from the upstream reader.
2. Hash the bytes to derive their content key.
3. In expiring mode, claim a renewable publication marker for the content key.
4. Write the bytes under their content key.
5. Record the object generation and content key in the offset mapping.
6. Replace the publication marker with the content expiration.

Publishing the mapping after the bytes makes an interrupted write a recoverable cache
miss instead of visible coverage without data. Reads also treat malformed mappings,
missing blobs, wrong-sized blobs, and stale generations as misses. Cleanup uses
`compare_exchange` so it cannot delete a concurrently replaced value.

## Invalidation

`SparseIO::invalidate` atomically advances an object's generation, then removes its old
offset mappings. In-flight reads carry the generation they observed and check it again
before publishing. A read racing invalidation can return source bytes to its caller,
but it cannot make stale coverage visible to later reads.

Invalidation removes object coverage, not shared content blobs. Permanent cache content
remains until the writer is managed externally. Expiring content becomes eligible for
garbage collection after no current mapping references it.

## Expiration and Garbage Collection

Permanent caching is the default and records no expiration metadata.
`CachePolicy::Expiring` refreshes a content expiration on materialization and warm
reads. Garbage collection is explicit through `SparseIO::collect_garbage`; SparseIO
does not start a timer or background task.

One collection pass acquires a metadata lease with `compare_exchange`, removes stale
mappings, discovers all currently referenced content, refreshes those expirations, and
deletes expired unreferenced blobs. Compare-and-set publication and deletion markers
coordinate slow writes with blob removal. Failed writer deletion restores the previous
expiration on a best-effort basis. Later collectors resume stale unreferenced markers
and recover referenced markers left by interrupted operations.

This lifecycle favors a low-latency read path. Collection performs the global scans and
is intentionally controlled by the embedding application.
