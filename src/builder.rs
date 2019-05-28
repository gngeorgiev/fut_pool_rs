use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

use crate::connector::Connector;
use crate::pool::{Pool, Peek};

pub struct PoolBuilder<T> where T: Peek {
    _connector: Option<Arc<Box<Connector<T>>>>,
}

impl<T> PoolBuilder<T> where T: Peek {
    pub fn new() -> PoolBuilder<T> {
        PoolBuilder { _connector: None }
    }

    pub fn connector(mut self, connector: Box<Connector<T>>) -> Self {
        self._connector = Some(Arc::new(connector));
        self
    }

    pub fn build(self) -> Pool<T> {
        Pool {
            connector: self._connector.unwrap(),
            items: Arc::new(RwLock::new(VecDeque::new())),
        }
    }
}
