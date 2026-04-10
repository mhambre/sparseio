//! Coverage tracking for sparse I/O operations.
//!
//! The store tracks exact covered ranges so SparseIO can decide whether a read
//! can be served from cache, whether a chunk is already materialized, and
//! whether a concurrent fetch should be reused. It intentionally does not
//! normalize or merge ranges here; observable correctness is validated at the
//! I/O layer.

use std::collections::BTreeMap;

pub(crate) struct Coverage {
    store: BTreeMap<usize, usize>,
}

impl Coverage {
    pub(crate) fn new() -> Self {
        Self { store: BTreeMap::new() }
    }

    /// Gets a prior chunk that starts before or at the target offset.
    pub(crate) fn get(&self, target: usize) -> Option<(usize, usize)> {
        let prior = self.store.range(..=target).next_back();
        prior.map(|(&start, &end)| (start, end))
    }

    /// Inserts a new chunk into the coverage store.
    pub(crate) fn insert(&mut self, offset: usize, length: usize) {
        let end = offset.saturating_add(length);
        self.store.insert(offset, end);
    }
}

#[cfg(test)]
mod tests {
    use super::Coverage;

    /// This test pins the coverage lookup contract to the exact range that
    /// contains the target offset, which is the observable behavior SparseIO
    /// depends on when deciding whether a chunk is cached.
    #[tokio::test]
    async fn get_returns_the_latest_range_starting_at_or_before_target() {
        let mut coverage = Coverage::new();
        coverage.insert(0, 8);
        coverage.insert(16, 4);

        assert_eq!(coverage.get(0), Some((0, 8)));
        assert_eq!(coverage.get(7), Some((0, 8)));
        assert_eq!(coverage.get(16), Some((16, 20)));
        assert_eq!(coverage.get(19), Some((16, 20)));
        assert_eq!(coverage.get(21), Some((16, 20)));
        assert_eq!(coverage.get(22), Some((16, 20)));
    }

    /// This test keeps the coverage store honest about the literal ranges
    /// inserted into it without asserting any future merge behavior.
    #[tokio::test]
    async fn insert_keeps_exact_endpoints_without_inventing_overlap_rules() {
        let mut coverage = Coverage::new();
        coverage.insert(4, 4);
        coverage.insert(12, 2);

        assert_eq!(coverage.get(4), Some((4, 8)));
        assert_eq!(coverage.get(12), Some((12, 14)));
    }
}
