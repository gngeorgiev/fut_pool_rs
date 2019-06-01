use std::io::{Error, ErrorKind, Result};
use std::pin::Pin;
use std::task::Context;
use std::time::{Duration, Instant};

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
    factory_future_in_progress: Option<Pin<Box<dyn Future<Output = Result<T>>>>>,
    backoff: BackoffStrategy,
    first_poll: bool,
    backoff_delay: Option<Delay>,
}

impl<T> PoolTaker<T>
where
    T: Connection,
{
    pub(crate) fn new(pool: Pool<T>) -> PoolTaker<T> {
        PoolTaker {
            started_at: Instant::now(),
            tries: 0,
            factory_future_in_progress: None,
            first_poll: true,
            backoff: pool.backoff.clone(),
            backoff_delay: None,
            pool,
        }
    }
}

unsafe impl<T> Send for PoolTaker<T> where T: Connection {}

unsafe impl<T> Sync for PoolTaker<T> where T: Connection {}

impl<T> PoolTaker<T>
where
    T: Connection,
{
    fn backoff_timeout(&mut self) -> Option<Duration> {
        match self.backoff {
            BackoffStrategy::Exponential(ref mut bo) => bo.next(),
            BackoffStrategy::Fibonacci(ref mut bo) => bo.next(),
            BackoffStrategy::Fixed(ref mut bo) => bo.next(),
            BackoffStrategy::None => None,
        }
    }
}

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
        }

        self.backoff_delay = None;
        self.first_poll = false;

        let mut available_object = self.pool.try_take();
        let object_in_progress = self.factory_future_in_progress.is_some();
        if available_object.is_none() && !object_in_progress {
            //1. get a connection from the pool connector

            self.factory_future_in_progress = Some((self.pool.factory)());
            cx.waker().wake_by_ref();
            Poll::Pending
        } else if let Some(ref mut factory_future) = self.factory_future_in_progress {
            //2. the connection we got from the connector is trying to connect
            //once it does, we put it in the pool

            let object = try_ready!(factory_future.as_mut().poll(cx));
            self.pool.put(object);
            self.factory_future_in_progress = None;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else if let Some(ref mut object) = available_object {
            //3. we have a connected connection, sometimes it's a brand new one
            //sometimes it's recycled. We need to test whether it's usable

            let mut connection = object.detach().unwrap();
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
                        let timeout = self.backoff_timeout();
                        if let Some(timeout) = timeout {
                            self.backoff_delay = Some(Delay::new(timeout));
                            match self.backoff_delay {
                                Some(ref mut delay) => ready!(delay.poll_unpin(cx))?,
                                None => unreachable!(),
                            };
                        }

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
