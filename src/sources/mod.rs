//! Example source implementations. Quick and dirty implementations of the [`crate::SourceReader`], and [`crate::ExtentStore`] for
//! dealing with common use cases (File -> Sparse Memory, File -> Sparse File, HTTP Range Reader -> Sparse File).
//!
//! While these can be used as-is, they are primarily intended as examples and starting points for building your own custom sources.

#[cfg(feature = "file")]
pub mod file;
#[cfg(feature = "http")]
pub mod http;
#[cfg(feature = "memory")]
pub mod memory;
