use std::io::{Error, ErrorKind, Result};
use std::pin::Pin;
use std::task::Context;
use std::time::Instant;

use futures::{ready, try_ready, Future, FutureExt, Poll};
use futures_timer::Delay;

use crate::backoff::BackoffStrategy;
use crate::connection::Connection;
use crate::guard::PoolGuard;
use crate::pool::Pool;

pub struct PoolTaker<T>
where
    T: Connection,
{
    pool: Pool<T>,
    started_at: Instant,
    tries: usize,
    connector_future_in_progress: Option<Pin<Box<dyn Future<Output = Result<T>>>>>,
    backoff: BackoffStrategy,
    first_poll: bool,
}

impl<T> PoolTaker<T>
where
    T: Connection,
{
    pub(crate) fn new(pool: Pool<T>) -> PoolTaker<T> {
        PoolTaker {
            started_at: Instant::now(),
            tries: 0,
            connector_future_in_progress: None,
            first_poll: true,
            backoff: pool.backoff.clone(),
            pool,
        }
    }
}

unsafe impl<T> Send for PoolTaker<T> where T: Connection {}

unsafe impl<T> Sync for PoolTaker<T> where T: Connection {}

impl<T> Future for PoolTaker<T>
where
    T: Connection,
{
    type Output = Result<PoolGuard<T>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if !self.first_poll {
            if self.pool.timeout.is_some() && self.started_at.elapsed() > self.pool.timeout.unwrap()
            {
                return Poll::Ready(Err(Error::from(ErrorKind::TimedOut)));
            }

            let timeout = match self.backoff {
                BackoffStrategy::Exponential(ref mut bo) => bo.next(),
                BackoffStrategy::Fibonacci(ref mut bo) => bo.next(),
                BackoffStrategy::Fixed(ref mut bo) => bo.next(),
                BackoffStrategy::None => None,
            };

            if let Some(timeout) = timeout {
                let mut delay = Delay::new(timeout);
                ready!(delay.poll_unpin(cx))?;
            }
        }

        self.first_poll = false;
        let mut available_connection = self.pool.try_take();
        let connection_is_in_progress = self.connector_future_in_progress.is_some();
        if available_connection.is_none() && !connection_is_in_progress {
            //1. get a connection from the pool connector

            self.connector_future_in_progress = Some((self.pool.factory)());
            cx.waker().wake_by_ref();
            Poll::Pending
        } else if let Some(ref mut connector_future) = self.connector_future_in_progress {
            //2. the connection we got from the connector is trying to connect
            //once it does, we put it in the pool

            let connection = try_ready!(connector_future.as_mut().poll(cx));
            self.pool.put(connection);
            self.connector_future_in_progress = None;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else if let Some(ref mut connection) = available_connection {
            //3. we have a connected connection, sometimes it's a brand new one
            //sometimes it's recycled. We need to test whether it's usable

            let mut connection = connection.detach().unwrap();
            match connection.test_poll(cx) {
                Poll::Ready(Ok(usable)) => {
                    if usable {
                        Poll::Ready(Ok(PoolGuard::new(connection, self.pool.clone())))
                    } else {
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                }

                Poll::Ready(Err(err)) => {
                    self.tries = self.tries + 1;
                    if self.pool.max_tries.is_some() && self.tries >= self.pool.max_tries.unwrap() {
                        Poll::Ready(Err(err))
                    } else {
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                }
                Poll::Pending => Poll::Pending,
            }
        } else {
            unreachable!("the pool is in an invalid state")
        }
    }
}
