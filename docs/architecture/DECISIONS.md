# Optimizations

This document outlines the general performance optimizations and design choices
that allow SparseIO to be efficient and effective.

## Shared Future Store

Consumers do not generally intend to make concurrent requests for the same upstream
chunk. It can still happen when downloads are multiplexed across distributed consumers,
even if only a few consumers remain in lockstep. Consumers that fall behind the upstream
read are likely to fall back to the cache.

To reduce cache, metadata, and upstream reads, SparseIO keeps an in-memory `HashMap` of
shared futures. This is the first structure a [`Viewer`](./OBJECTS.md#viewer) read checks.

The goal is to avoid repeating work another request has already started. As described
in the [read flow](./FLOW.md), the first requester for a chunk checks the
[`Metadata`](./API.md#metadata) store, reads the cache or upstream source, and publishes
any resulting state. A later requester can subscribe to the same future and avoid
repeating those operations.

When no future exists, the requester registers its work before continuing. Subsequent
readers subscribe to that future whether the chunk is ultimately returned from metadata,
cache, or the upstream source.

This introduces some lock contention, but the critical section is small and prevents
an entire chunk read from being duplicated.

## Expiry-Based CAS Lifecycle

For [CAS](./CAS.md) lifecycle management, SparseIO uses expiry timestamps because they
fit the metadata backends the library is intended to support. Locks, leases, and
backend-specific update primitives would either add latency to the normal read/write
path or make the core API depend on coordination features that many reasonable
key-value stores may not provide.

Instead each cached chunk gets an expiry timestamp. Insertion and cache hits just set that timestamp
to `now + cache_lifetime`, and deletion of a file only needs to remove its metadata mappings. This keeps
normal insertion `O(1)` and normal deletion `O(1)`, while GC handles the slower work by scanning mappings,
refreshing referenced chunks, and removing expired chunks in the background.

This means stale mappings are acceptable: if metadata points to a chunk that has already expired or been
removed, the read path treats it as a cache miss and fetches from upstream again. See [CAS](./CAS.md) for
the full CAS model.
