//! Implementations of the [`crate::Reader`], and [`crate::Writer`] for dealing with common use cases. Usages of
//! such sources are shown in the examples, and they can be used as building blocks for more complex custom sources,
//! references for building your own source, or as-is.

#[cfg(feature = "file")]
pub mod file;
#[cfg(feature = "http")]
pub mod http;
