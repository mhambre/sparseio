# Construction

## `SparseIO`

`SparseIO` coordinates an upstream [`Reader`](./API.md#reader), a cache
[`Writer`](./API.md#writer), and a [`Metadata`](./API.md#metadata) index. Instances are
cheap to clone because immutable configuration and in-flight work are shared.

Construction uses `SparseIO::builder()` and requires:

- A `Writer` implementation.
- A `Metadata` implementation.
- A `ReaderRegistry` containing at least the URI schemes the application opens.

Chunk size and per-read chunk concurrency are immutable after construction. The
defaults are 64 KiB and 16 operations. Permanent caching is the default; expiring
caching is opt-in through `cache::CachePolicy`.

## `ReaderRegistry`

`ReaderRegistry` maps case-insensitive URI schemes to object-safe `ReaderFactory`
trait objects. `SparseIO::open` passes the complete canonical URI to the selected
factory, which constructs an object-bound `Reader`. This keeps serialized identities
independent from concrete readers and lets `register_many` share one factory under
several schemes.

Applications must keep every scheme referenced by their metadata available. Registering
a scheme again replaces its prior factory.

## `Viewer`

`SparseIO::open("service://path/to/object")` returns a cloneable `Viewer`. Opening is
synchronous because registry factories construct readers synchronously; backend I/O
begins when an async method is polled.

The primary methods are:

- `read_at(offset, length)`: asynchronously read a byte window, truncated at EOF.
- `len()`: asynchronously load and cache the stable object length for this viewer.
- `is_empty()`: report whether the object has zero bytes.
- `bytestream()`: return an executor-neutral futures `Stream` of ordered chunks.

`offset == len` is valid and returns empty bytes. An offset beyond EOF and arithmetic
overflow return structured `sparseio::Error` values.
