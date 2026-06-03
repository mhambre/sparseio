# Construction

## SparseIO Object

The SparseIO object is the main interface for users of the library. It provides the
backing logic management behind the interaction between the `Reader`, `Writer`,
and `Metadata` objects to maintain the abstraction from the user.

When a user creates a SparseIO object, they do it through a SparseIOBuilder, which
is a [builder pattern](https://www.lurklurk.org/effective-rust/builders.html) making
it easy to construct the object without having to worry about defaults.

Three things are required to construct a SparseIO object:

- A [Writer](./API.md#Writer) implementation
- A [Metadata](./API.md#Metadata) implementation
- A [ReaderRegistry](#ReaderRegistry) implementation

There are some other tunable parameters that can be set in the builder, such as the
`chunk_size`, control over prefetching behavior, etc. However these are explained a
bit further in the [SparseIOBuilder docs](http://docs.rs/sparseio/latest/sparseio/struct.SparseIOBuilder.html).

## ReaderRegistry

An important design decision of SparseIO was how to cleanly support the construction
and reconstruction of SparseIO objects when our metadata must be serializable. To
overcome this challenge we designed a type called a `ReaderRegistry`, which is
essentially a mapping of a `Reader` implementation to a recipe for reconstruction
based on serialized metadata.

This does leave it up to the user to ensure that they properly manage the `ReaderRegistry`
to ensure that existing metadata is fully supported by all `Reader` implementations
in the registry.

This also defines how users do interact with the `Reader` itself when attempting to
read an object. For example, if a user constructed a `Reader` dedicated to reading from their
website that just takes raw paths from a URI defined explicitly in their reader the
registry key could be "mysite". As such, the canonicalized path for interacting with this
reader would be `mysite+/path/to/object`. This allows users to easily manage multiple readers
with different sources in one SparseIO object.

## Viewer

The Viewer is a non-user contructable object but is one of the most interacted with
objects in our architecture. It provides the API abstracting the grunt-work we
manage behind the scenes in the interactions between the `Reader`, `Writer`, and `Metadata`
objects.

When a user attempts to read an object through the `SparseIO` object through `SparseIO::open`
(e.g. `open("mysite+/path/to/object")`) they are actually getting back a `Viewer` object.
The Viewer provides a couple of core methods for interacting with the data including:

- `read(offset: usize, length: usize) -> io::Result<Bytes>`: Read a byte range from the object.
- `len() -> usize`: Get the total length of the object in bytes.
- `to_bytestream() -> impl Stream<Item = io::Result<Bytes>>`: Convert the Viewer to a bytestream for easier integration with async applications.
