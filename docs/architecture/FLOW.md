# Flow

In this section we will go over the flow of how a read request is processed in SparseIO.

When a user initially constructs a SparseIO object they are required to provide a `Metadata`
implementation, a `Writer` implementation, and a `ReaderRegistry` implementation loaded with
the `Reader` implementations they want to use. This is all done through the `SparseIOBuilder` object, which
provides a nice interface for constructing the `SparseIO` object without having to worry about defaults.

Users can then call `SparseIO::open` with a canonicalized path to get back a `Viewer` object.
This looks something like `instance.open(mysite+/path/to/object)`. The main component of the
viewer object is the `read_at()` method because it provides the core functionality
behind the bytestream, and general reads.

When a user calls `read_at()` with an offset and length, we first normalize the offset
to be a multiple of the chunk size (i.e. with a chunk size of 4KB, an offset of 5KB goes
to 4KB). This means the metadata key for that offset will be `metadata:{uri hash}:{normalized offset}`.
Using that key we check in our metadata store to see if we have a mapping for that offset.
If we do have a mapping, we can use our `Writer` implementation to read the chunk from cache
and build up the response. We keep doing this for every chunk until `offset + length` rounded
up to chunk size.

## Cache Miss

In the event of a cache miss, we use the `ReaderRegistry` to find the appropriate
`Reader` implementation (this happens at the `Viewer` instantiation). We then are able
to use the `Reader` to read the chunk of data from the source using its `read_at()`.

To prevent deduplicate requests for the same chunk we use an in memory flighting system
that stores shared futures for each chunk s.t. directly concurrent requests can avoid the
latency of reading the metadata store and can directly await the same future for the chunk.

Once the parent reader recieves the chunk of data from the source, we are able to
return the chunk to the user while also calculating the hash of the chunk and writing
it to the cache using our `Writer` implementation and the identifier `cache:{hash}`.
We can then write the mapping for the metadata key for that offset (`metadata:{uri hash}:{normalized offset}`)
to the cache key (`cache:{hash}`) for future reads.

Along with the actual chunk cache we also write a reference count key for the chunk
in the metadata store with the key `refcount:{hash}`. This allows us to keep track of
how many metadata keys are pointing to that chunk in cache, which is crucial for our
eviction policy as we can only safely evict a chunk from cache when there are no
metadata keys pointing to it.
