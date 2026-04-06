//! Coverage tracking for sparse I/O operations. This module is responsible for tracking which logical chunks of a
//! sparse object are filled in, working, etc. This is used to determine if we can coallesce adjacent chunks together,
//! and to determine if a read should wait for an in-flight write to complete.

use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub(crate) struct Coverage {
    store: Arc<RwLock<BTreeMap<usize, usize>>>,
}

impl Coverage {
    pub(crate) fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Gets a prior chunk that starts before or at the target offset.
    pub(crate) async fn get(&self, target: usize) -> Option<(usize, usize)> {
        let map = self.store.read().await;
        let prior = map.range(..=target).next_back();
        prior.map(|(&start, &end)| (start, end))
    }

    /// Inserts a new chunk into the coverage store. This will coallesce with adjacent chunks if they exist.
    pub(crate) async fn insert(&self, offset: usize, length: usize) {
        let mut map = self.store.write().await;
        let end = offset.saturating_add(length);
        map.insert(offset, end);
    }
}
