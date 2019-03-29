use futures::{task, try_ready, Async, Future, Poll};
use std::collections::VecDeque;
use std::io;
use std::ops::Deref;
use std::sync::{Arc, RwLock};

use crate::builder::PoolBuilder;
use crate::connector::Connector;

pub struct Pool<T> {
    pub(crate) connector: Arc<Box<Connector<T>>>,

    pub(crate) items: Arc<RwLock<VecDeque<T>>>,
}

impl<T> Clone for Pool<T> {
    fn clone(&self) -> Self {
        Pool {
            connector: self.connector.clone(),
            items: self.items.clone(),
        }
    }
}

impl<T> Pool<T> {
    pub fn builder() -> PoolBuilder<T> {
        PoolBuilder::new()
    }

    pub fn take(&self) -> PoolTaker<T> {
        PoolTaker::new(self.clone())
    }

    pub fn try_take(&self) -> Option<PoolGuard<T>> {
        let items = self.items.clone();
        let mut items = items.write().unwrap();
        if let Some(item) = items.pop_front() {
            Some(PoolGuard::new(item, self.clone()))
        } else {
            None
        }
    }

    pub fn put(&self, item: T) {
        let mut items = self.items.write().unwrap();
        items.push_back(item);
    }

    pub fn size(&self) -> usize {
        self.items.read().unwrap().len()
    }
}

pub struct PoolTaker<T> {
    pool: Pool<T>,

    connector_future: Option<Box<Future<Item = T, Error = io::Error>>>,
}

unsafe impl<T> Send for PoolTaker<T> {}
unsafe impl<T> Sync for PoolTaker<T> {}

impl<T> PoolTaker<T> {
    fn new(pool: Pool<T>) -> PoolTaker<T> {
        PoolTaker {
            pool,
            connector_future: None,
        }
    }
}

impl<T> Future for PoolTaker<T> {
    type Item = PoolGuard<T>;

    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Some(ref mut connector_future) = self.connector_future {
            let item = try_ready!(connector_future.poll());
            Ok(Async::Ready(PoolGuard::new(item, self.pool.clone())))
        } else {
            if let Some(item) = self.pool.try_take() {
                Ok(Async::Ready(item))
            } else {
                self.connector_future = Some((self.pool.connector)());
                task::current().notify();
                Ok(Async::NotReady)
            }
        }
    }
}

pub struct PoolGuard<T> {
    inner: Option<T>,
    pool: Pool<T>,
}

impl<T> PoolGuard<T> {
    fn new(item: T, pool: Pool<T>) -> PoolGuard<T> {
        PoolGuard {
            inner: Some(item),
            pool,
        }
    }
}

impl<T> PoolGuard<T> {
    pub fn detach(&mut self) -> Option<T> {
        let item = self.inner.take();
        item
    }
}

impl<T> Deref for PoolGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner.as_ref().expect("deref PoolGuard no inner")
    }
}

impl<T> Drop for PoolGuard<T> {
    fn drop(&mut self) {
        if let Some(item) = self.inner.take() {
            self.pool.put(item);
        }
    }
}
