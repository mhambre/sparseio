//! Builder for constructing [`SparseIO`](crate::SparseIO) instances.

use std::io;
use std::sync::Arc;

use crate::{Metadata, ReaderRegistry, SparseIO, Writer};

/// Builder for the main SparseIO coordinator.
#[derive(Default)]
pub struct Builder {
    writer: Option<Arc<dyn Writer>>,
    metadata: Option<Arc<dyn Metadata>>,
    registry: Option<Arc<ReaderRegistry>>,

    chunk_size: Option<usize>,
}

impl Builder {
    /// Create a new builder with default tunables.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the cache writer implementation.
    pub fn writer(mut self, writer: impl Writer + 'static) -> Self {
        self.writer = Some(Arc::new(writer));
        self
    }

    /// Set the cache writer implementation from a shared trait object.
    pub fn writer_arc(mut self, writer: Arc<dyn Writer>) -> Self {
        self.writer = Some(writer);
        self
    }

    /// Set the metadata store implementation.
    pub fn metadata(mut self, metadata: impl Metadata + 'static) -> Self {
        self.metadata = Some(Arc::new(metadata));
        self
    }

    /// Set the metadata store implementation from a shared trait object.
    pub fn metadata_arc(mut self, metadata: Arc<dyn Metadata>) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set the reader registry object.
    pub fn registry(mut self, registry: ReaderRegistry) -> Self {
        self.registry = Some(Arc::new(registry));
        self
    }

    /// Set the reader registry implementation from a shared object.
    pub fn registry_arc(mut self, reader_registry: Arc<ReaderRegistry>) -> Self {
        self.registry = Some(reader_registry);
        self
    }

    /// Set the immutable chunk size for this SparseIO instance.
    pub fn chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = Some(chunk_size);
        self
    }

    /// Build a SparseIO instance.
    pub fn build(self) -> io::Result<SparseIO> {
        if self.writer.is_none() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Writer implementation is required"));
        }

        if self.metadata.is_none() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Metadata implementation is required"));
        }

        if self.registry.is_none() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "ReaderRegistry implementation is required"));
        }

        Ok(SparseIO {
            writer: self.writer.expect("writer should be set"),
            metadata: self.metadata.expect("metadata should be set"),
            registry: self.registry.expect("registry should be set"),
            chunk_size: self.chunk_size.unwrap_or(crate::globals::DEFAULT_CHUNK_SIZE),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::Arc;

    use super::Builder;
    use crate::ReaderRegistry;
    use crate::utils::metadatas::MockMetadata;
    use crate::utils::readers::MockReader;
    use crate::utils::writers::MockWriter;

    fn writer() -> MockWriter {
        MockWriter::new()
    }

    fn metadata() -> MockMetadata {
        MockMetadata::new()
    }

    fn registry() -> ReaderRegistry {
        let mut registry = ReaderRegistry::new();
        registry.register("stub", MockReader::new(bytes::Bytes::new()));
        registry
    }

    fn assert_invalid_input(result: io::Result<crate::SparseIO>, expected_message: &str) {
        let error = match result {
            Ok(_) => panic!("builder should reject missing required input"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(error.to_string(), expected_message);
    }

    #[test]
    fn build_requires_writer() {
        let result = Builder::new().metadata(metadata()).registry(registry()).build();

        assert_invalid_input(result, "Writer implementation is required");
    }

    #[test]
    fn build_requires_metadata() {
        let result = Builder::new().writer(writer()).registry(registry()).build();

        assert_invalid_input(result, "Metadata implementation is required");
    }

    #[test]
    fn build_requires_registry() {
        let result = Builder::new().writer(writer()).metadata(metadata()).build();

        assert_invalid_input(result, "ReaderRegistry implementation is required");
    }

    #[test]
    fn build_uses_default_chunk_size() {
        let sparseio = Builder::new()
            .writer(writer())
            .metadata(metadata())
            .registry(registry())
            .build()
            .expect("builder should accept all required inputs");

        assert_eq!(sparseio.chunk_size, crate::globals::DEFAULT_CHUNK_SIZE);
    }

    #[test]
    fn build_uses_configured_chunk_size() {
        let sparseio = Builder::new()
            .writer(writer())
            .metadata(metadata())
            .registry(registry())
            .chunk_size(8192)
            .build()
            .expect("builder should accept custom chunk size");

        assert_eq!(sparseio.chunk_size, 8192);
    }

    #[test]
    fn build_accepts_arc_inputs() {
        let sparseio = Builder::new()
            .writer_arc(Arc::new(writer()))
            .metadata_arc(Arc::new(metadata()))
            .registry_arc(Arc::new(registry()))
            .build()
            .expect("builder should accept shared inputs");

        assert_eq!(sparseio.chunk_size, crate::globals::DEFAULT_CHUNK_SIZE);
    }
}
