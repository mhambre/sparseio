//! Reader registry abstractions.

use std::collections::HashMap;

use crate::Reader;

/// Registry for storing SparseIO reader implementations.
#[derive(Default)]
pub struct ReaderRegistry {
    readers: HashMap<String, Box<dyn Reader + Send + Sync>>,
}

impl ReaderRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            readers: HashMap::new(),
        }
    }

    /// Register a reader implementation under a canonical name.
    pub fn register(&mut self, name: impl Into<String>, reader: impl Reader + 'static) {
        self.readers.insert(name.into(), Box::new(reader));
    }

    /// Retrieve a reader implementation by its canonical name.
    pub fn get(&self, name: &str) -> Option<&(dyn Reader + Send + Sync)> {
        self.readers.get(name).map(|r| r.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use bytes::Bytes;

    use super::ReaderRegistry;
    use crate::Reader;
    use crate::utils::readers::StubReader;

    struct LenReader(usize);

    impl Reader for LenReader {
        fn len(&self) -> io::Result<usize> {
            Ok(self.0)
        }

        fn read_at(&self, _offset: usize, _length: usize) -> io::Result<Bytes> {
            unimplemented!("registry tests should not call reader read_at")
        }
    }

    #[test]
    fn new_starts_empty() {
        let registry = ReaderRegistry::new();

        assert!(registry.get("missing").is_none());
    }

    #[test]
    fn default_starts_empty() {
        let registry = ReaderRegistry::default();

        assert!(registry.get("missing").is_none());
    }

    #[test]
    fn register_makes_reader_retrievable_by_name() {
        let mut registry = ReaderRegistry::new();

        registry.register("stub", StubReader);

        assert!(registry.get("stub").is_some());
        assert!(registry.get("other").is_none());
    }

    #[test]
    fn register_replaces_existing_reader_for_name() {
        let mut registry = ReaderRegistry::new();

        registry.register("source", LenReader(10));
        registry.register("source", LenReader(20));

        let reader = registry.get("source").expect("replacement reader should be registered");

        assert_eq!(reader.len().expect("len should be readable"), 20);
    }
}
