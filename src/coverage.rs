//! Coverage tracking for sparse I/O operations. This module is responsible for tracking which logical chunks of a
//! sparse object are filled in, working, etc. This is used to determine if we can coallesce adjacent chunks together,
//! and to determine if a read should wait for an in-flight write to complete.

use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub(crate) struct Coverage {
    _store: Arc<RwLock<BTreeMap<usize, usize>>>,
}

impl Coverage {
    pub(crate) fn new() -> Self {
        Self {
            _store: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}
