//! In-memory implementation of the sparse I/O extent store. Be careful when using this implementation, as it will
//! consume memory proportional to the size of the sparse object being read. This is intended for testing and demonstration
//! purposes only, and should not be used in production. for smaller files is is possible to use this along with MergeExt
//! for the ExtentStore which will merge the object so metadata is no longer needed.
