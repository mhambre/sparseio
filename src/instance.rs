//! Core instance struct and related functions.
//!
//! This module defines the `SparseIO` object, which represents a single
//! instance of our management interface for the entire sparse file read system.

use std::io;
use std::sync::Arc;

use crate::viewer::Viewer;
use crate::{Metadata, ReaderRegistry, Writer};

/// Coordinator for sparse, chunked reads and cache materialization.
#[allow(dead_code)]
#[derive(Clone)]
pub struct SparseIO {
    // World interface components
    pub(crate) writer: Arc<dyn Writer>,
    pub(crate) metadata: Arc<dyn Metadata>,
    pub(crate) registry: Arc<ReaderRegistry>,

    // Tunable States
    pub(crate) chunk_size: usize,
}

impl SparseIO {
    /// Create a builder for configuring a SparseIO instance.
    pub fn builder() -> crate::builder::Builder {
        crate::builder::Builder::new()
    }

    /// Open a canonical SparseIO path and return a viewer over the object.
    pub fn open(&self, path: impl AsRef<str>) -> io::Result<Viewer> {
        let _ = path;
        todo!("resolve reader and create viewer")
    }
}
