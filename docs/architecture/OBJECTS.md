# Construction

## `SparseIO`

The `SparseIO` object is the main interface for users of the library. It provides the
backing logic behind interactions between [`Reader`](./API.md#reader),
[`Writer`](./API.md#writer), and [`Metadata`](./API.md#metadata) implementations while
keeping those details abstracted from the user.

Users create a `SparseIO` instance through a `Builder`, which follows the
[builder pattern](https://rust-unofficial.github.io/patterns/patterns/creational/builder.html)
and makes the object easy to construct without requiring users to manage defaults.

Three things are required to construct a `SparseIO` instance:

- A [`Writer`](./API.md#writer) implementation.
- A [`Metadata`](./API.md#metadata) implementation.
- A [`ReaderRegistry`](#readerregistry) implementation.

The builder also configures tunable parameters such as `chunk_size`. See the
[`Builder` API][builder-docs] for the complete configuration surface.

## `ReaderRegistry`

An important design decision is how to cleanly support constructing and reconstructing
`SparseIO` objects while keeping metadata serializable. `ReaderRegistry` maps a
`Reader` implementation to a recipe for reconstruction based on serialized metadata.

This leaves users responsible for managing the `ReaderRegistry` so existing metadata
remains supported by every `Reader` implementation in the registry.

The registry also defines how users select a `Reader` when opening an object. For
example, a `Reader` dedicated to a website could be registered under `service`, making
its canonical object path `service://path/to/object`. This allows one `SparseIO`
instance to manage readers backed by several sources.

To support reconstruction, the `Reader` trait also requires implementations of
`From<&str>` and `From<String>`. Each conversion receives the object path after the
registry identifier has been removed.

## `Viewer`

`Viewer` cannot be constructed directly by users, but it is one of the most frequently
used objects in the architecture. It hides the work performed among the `Reader`,
`Writer`, and `Metadata` implementations.

Calling `SparseIO::open`, for example `instance.open("service://path/to/object")`,
returns a `Viewer`. Its core methods are:

- `read_at(&self, offset: usize, length: usize) -> io::Result<Bytes>`: Read a byte range from the object.
- `len(&self) -> io::Result<usize>`: Get the total length of the object in bytes.
- `bytestream(&self) -> ByteStream`: Convert the `Viewer` to a byte stream for easier application integration.

[builder-docs]: https://docs.rs/sparseio/latest/sparseio/struct.Builder.html
