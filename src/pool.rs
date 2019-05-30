use std::collections::VecDeque;
use std::io::Result;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;

use crate::builder::PoolBuilder;
use crate::connection::Connection;
use crate::connector::Connector;
use crate::guard::PoolGuard;
use crate::taker::PoolTaker;

pub struct Pool<T>
where
    T: Connection,
{
    pub(crate) connector: Arc<Connector<T>>,
    pub(crate) connections: Arc<RwLock<VecDeque<T>>>,

    pub(crate) timeout: Option<Duration>,
    pub(crate) max_tries: Option<usize>,
    pub(crate) capacity: Option<usize>,
}

impl<T> Clone for Pool<T>
where
    T: Connection,
{
    fn clone(&self) -> Self {
        Pool {
            connector: self.connector.clone(),
            connections: self.connections.clone(),
            timeout: self.timeout.clone(),
            max_tries: self.max_tries.clone(),
            capacity: self.capacity.clone(),
        }
    }
}

impl<T> Pool<T>
where
    T: Connection,
{
    pub fn builder() -> PoolBuilder<T> {
        PoolBuilder::new()
    }

    pub async fn take(&self) -> Result<PoolGuard<T>> {
        PoolTaker::new(self.clone()).await
    }

    pub fn try_take(&self) -> Option<PoolGuard<T>> {
        let mut connections = self.connections.write();
        let conn = connections.pop_front()?;
        Some(PoolGuard::new(conn, self.clone()))
    }

    pub fn put(&self, connection: T) {
        let mut connections = self.connections.write();
        let capacity = self.capacity.unwrap_or_else(|| 0);
        if capacity > 0 && connections.len() >= capacity {
            connections.pop_back();
        }

        connections.push_back(connection);
    }

    pub fn size(&self) -> usize {
        self.connections.read().len()
    }
}
