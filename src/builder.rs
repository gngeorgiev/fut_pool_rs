use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use futures::Future;
use parking_lot::RwLock;
use tokio::io::Result;

use crate::connection::Connection;
use crate::connector::Connector;
use crate::pool::Pool;

pub struct PoolBuilder<T>
where
    T: Connection,
{
    _connector: Option<Arc<Connector<T>>>,
    _timeout: Option<Duration>,
    _max_tries: Option<usize>,
    _capacity: Option<usize>,
}

impl<T> PoolBuilder<T>
where
    T: Connection,
{
    pub fn new() -> PoolBuilder<T> {
        PoolBuilder {
            _connector: None,
            _timeout: Some(Duration::from_secs(10)),
            _max_tries: Some(10),
            _capacity: None,
        }
    }

    pub fn connector<F>(mut self, connector: impl Fn() -> F + Send + Sync + 'static) -> Self
    where
        F: Future<Output = Result<T>> + 'static,
    {
        self._connector = Some(Arc::new(move || Box::pin(connector())));
        self
    }

    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self._timeout = timeout;
        self
    }

    pub fn max_tries(mut self, max_tries: Option<usize>) -> Self {
        self._max_tries = max_tries;
        self
    }

    pub fn capacity(mut self, capacity: Option<usize>) -> Self {
        self._capacity = capacity;
        self
    }

    pub fn build(self) -> Pool<T> {
        let container_capacity = self._capacity.unwrap_or_else(|| 10);

        Pool {
            connector: self._connector.expect("A pool connector is required"),
            connections: Arc::new(RwLock::new(VecDeque::with_capacity(container_capacity))),
            timeout: self._timeout,
            max_tries: self._max_tries,
            capacity: self._capacity,
        }
    }
}
