use futures::{task, try_ready, Async, Future, Poll};
use smallvec::SmallVec;
use std::collections::VecDeque;
use std::io;
use std::ops::Deref;
use std::sync::{Arc, RwLock};
use tokio::io::Error;
use tokio::net::TcpStream;

use crate::builder::PoolBuilder;
use crate::connector::Connector;
use core::borrow::Borrow;

pub trait Peek {
    fn poll_peek(&mut self, buf: &mut [u8]) -> Poll<usize, Error>;
}

impl Peek for TcpStream {
    fn poll_peek(&mut self, buf: &mut [u8]) -> Poll<usize, Error> {
        self.poll_peek(buf)
    }
}

pub struct Pool<T>
where
    T: Peek,
{
    pub(crate) connector: Arc<Connector<T>>,

    pub(crate) items: Arc<RwLock<VecDeque<T>>>,
}

impl<T> Clone for Pool<T>
where
    T: Peek,
{
    fn clone(&self) -> Self {
        Pool {
            connector: self.connector.clone(),
            items: self.items.clone(),
        }
    }
}

impl<T> Pool<T>
where
    T: Peek,
{
    pub fn builder() -> PoolBuilder<T> {
        PoolBuilder::new()
    }

    pub fn take(&self) -> PoolTaker<T> {
        PoolTaker::new(self.clone())
    }

    pub fn try_take(&self) -> Option<T> {
        let items = self.items.clone();
        let mut items = items.write().unwrap();
        items.pop_front()
    }

    pub fn put(&self, item: T) {
        let mut items = self.items.write().unwrap();
        items.push_back(item);
    }

    pub fn size(&self) -> usize {
        self.items.read().unwrap().len()
    }
}

pub struct PoolTaker<T>
where
    T: Peek,
{
    buf: SmallVec<[u8; 1]>,
    pool: Pool<T>,
    connector_future: Option<Box<Future<Item = T, Error = io::Error>>>,
}

unsafe impl<T> Send for PoolTaker<T> where T: Peek {}
unsafe impl<T> Sync for PoolTaker<T> where T: Peek {}

impl<T> PoolTaker<T>
where
    T: Peek,
{
    fn new(pool: Pool<T>) -> PoolTaker<T> {
        PoolTaker {
            buf: SmallVec::new(),
            pool,
            connector_future: None,
        }
    }
}

impl<T> Future for PoolTaker<T>
where
    T: Peek,
{
    type Item = T;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Some(ref mut connector_future) = self.connector_future {
            let item = try_ready!(connector_future.poll());
            return Ok(Async::Ready(item));
        }

        if let Some(mut item) = self.pool.try_take() {
            match item.poll_peek(self.buf.as_mut_slice()) {
                Ok(Async::NotReady) => {
                    task::current().notify();
                    Ok(Async::NotReady)
                }
                Ok(Async::Ready(b)) => {
                    if b == 0 {
                        task::current().notify();
                        Ok(Async::NotReady)
                    } else {
                        Ok(Async::Ready(item))
                    }
                }
                Err(err) => {
                    task::current().notify();
                    Ok(Async::NotReady)
                }
            }
        } else {
            self.connector_future = Some((self.pool.connector)());
            task::current().notify();
            Ok(Async::NotReady)
        }
    }
}

//pub struct PoolGuard<T> where T: Peek {
//    inner: Option<T>,
//    pool: Pool<T>,
//}
//
//impl<T> PoolGuard<T> where T: Peek {
//    fn new(item: T, pool: Pool<T>) -> PoolGuard<T> {
//        PoolGuard {
//            inner: Some(item),
//            pool,
//        }
//    }
//}

//impl<T> PoolGuard<T> where T: Peek {
//    pub fn detach(&mut self) -> Option<T> {
//        let item = self.inner.take();
//        item
//    }
//}
//
//impl<T> Deref for PoolGuard<T> where T: Peek {
//    type Target = T;
//
//    fn deref(&self) -> &Self::Target {
//        &self.inner.as_ref().expect("deref PoolGuard no inner")
//    }
//}
//
//impl<T> Drop for PoolGuard<T> where T: Peek {
//    fn drop(&mut self) {
//        if let Some(item) = self.inner.take() {
//            self.pool.put(item);
//        }
//    }
//}
