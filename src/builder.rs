use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, RwLock};

use futures::Future;

use crate::connector::Connector;
use crate::pool::{Peek, Pool};

pub struct PoolBuilder<T>
where
    T: Peek,
{
    _connector: Option<Arc<Connector<T>>>,
}

impl<T> PoolBuilder<T>
where
    T: Peek,
{
    pub fn new() -> PoolBuilder<T> {
        PoolBuilder { _connector: None }
    }

    pub fn connector<F>(mut self, connector: impl Fn() -> F + 'static + Send + Sync) -> Self
    where
        F: Future<Item = T, Error = io::Error> + 'static + Send + Sync,
    {
        self._connector = Some(Arc::new(move || Box::new(connector())));
        self
    }

    pub fn build(self) -> Pool<T> {
        Pool {
            connector: self._connector.expect("connector is required"),
            items: Arc::new(RwLock::new(VecDeque::new())),
        }
    }
}
