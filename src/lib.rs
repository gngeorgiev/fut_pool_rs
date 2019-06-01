#![feature(async_await)]

mod builder;
mod factory;
mod connection;
mod pool;
mod taker;
mod guard;
mod backoff;

#[macro_use]
mod util;

mod tcp;

pub use crate::builder::PoolBuilder;
pub use crate::connection::Connection;
pub use crate::pool::{Pool};
pub use crate::guard::PoolGuard;
pub use crate::taker::PoolTaker;
pub use crate::backoff::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::Context;
    use std::time::{Duration, Instant};
    use futures::Poll;
    use std::io::{Result, Error, ErrorKind};

    macro_rules! tokio_run_async {
        ($e:expr) => {
            use futures::{FutureExt, TryFutureExt};

            let mut rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on($e.unit_error().boxed().compat()).unwrap();
        };
    }

    #[derive(Debug, Clone)]
    struct TcpConn(bool);

    impl Connection for TcpConn {
        fn test_poll(&mut self, _: &mut Context) -> Poll<Result<bool>> {
            Poll::Ready(Ok(self.0))
        }
    }

    #[test]
    fn default_values() {
        let pool = Pool::<TcpConn>::builder()
            .connector(|| futures::future::ok(TcpConn(true)))
            .build();

        assert_eq!(0, pool.size());
        assert_eq!(true, pool.try_take().is_none());
    }

    #[test]
    fn connect_1() {
        let pool = Pool::<TcpConn>::builder()
            .connector(|| futures::future::ok(TcpConn(true)))
            .build();

        let p = pool.clone();
        let fut = async move {
            p.take().await.unwrap();
        };
        tokio_run_async!(fut);

        assert_eq!(1, pool.size());
        assert_eq!(true, pool.try_take().is_some());
    }

    #[test]
    fn connect_2_take_detach_one() {
        let pool = Pool::<TcpConn>::builder()
            .connector(|| futures::future::ok(TcpConn(true)))
            .build();

        let fut = async move {
            let conn = pool.take().await.unwrap();
            pool.put(conn.clone());
            pool.put(conn.clone());

            assert_eq!(2, pool.size());
            let conn = pool.try_take();
            assert_eq!(true, conn.is_some());
            pool.put(conn.unwrap().detach().unwrap());

            let mut conn1 = pool.try_take().unwrap();
            assert_eq!(1, pool.size());
            pool.put(conn1.detach().unwrap());
            assert_eq!(2, pool.size());

            let item = pool.try_take().unwrap();
            assert_eq!(1, pool.size());
            drop(item);
            assert_eq!(2, pool.size());
        };
        tokio_run_async!(fut);
    }

    #[test]
    fn connect_10_times_connector_calls_dont_reuse_connections() {
        //convert this to integer_atomics when it lands
        use std::sync::{Arc, Mutex};

        let c = Arc::new(Mutex::new(0));
        let cc = c.clone();
        let pool = Pool::<TcpConn>::builder()
            .connector(move || {
                let cc = cc.clone();
                let mut cc = cc.lock().unwrap();
                *cc = (*cc) + 1;
                futures::future::ok(TcpConn(true))
            })
            .build();

        let fut = async move{
            for _ in 0..10 {
                pool.take().await.unwrap().detach();
            }
        };
        tokio_run_async!(fut);
        assert_eq!(10, *c.lock().unwrap());
    }

    #[test]
    fn connect_10_times_connector_calls_reuse_connections() {
        //convert this to integer_atomics when it lands
        use std::sync::{Arc, Mutex};

        let c = Arc::new(Mutex::new(0));
        let cc = c.clone();
        let pool = Pool::<TcpConn>::builder()
            .connector(move || {
                let cc = cc.clone();
                let mut cc = cc.lock().unwrap();
                *cc = (*cc) + 1;
                futures::future::ok(TcpConn(true))
            })
            .build();

        let fut = async move{
            for _ in 0..10 {
                //the connections are returned to the pool after the iteration ends
                pool.take().await.unwrap();
            }
        };
        tokio_run_async!(fut);
        assert_eq!(1, *c.lock().unwrap());
    }

    #[test]
    fn connect_timeout() {
        let pool = Pool::<TcpConn>::builder()
            .connector(|| futures::future::ok(TcpConn(false)))
            .timeout(Some(Duration::from_millis(100)))
            .build();

        let fut = async move {
            match pool.take().await {
                Ok(_) => panic!("should not work"),
                Err(err) => assert_eq!(err.kind(), ErrorKind::TimedOut),
            };
        };
        tokio_run_async!(fut);
    }

    #[test]
    fn capacity_2() {
        let pool = Pool::<TcpConn>::builder()
            .connector(|| futures::future::ok(TcpConn(true)))
            .capacity(Some(2))
            .build();

        let fut = async move {
            let mut v = vec![];
            for _ in 0..5 {
                v.push(pool.take().await.unwrap().detach().unwrap());
            }

            v.into_iter().for_each(|c| pool.put(c));
            assert_eq!(pool.size(), 2);
        };
        tokio_run_async!(fut);
    }

    #[test]
    fn capacity_none() {
        let pool = Pool::<TcpConn>::builder()
            .connector(|| futures::future::ok(TcpConn(true)))
            .capacity(None)
            .build();

        let fut = async move {
            let mut v = vec![];
            for _ in 0..5 {
                v.push(pool.take().await.unwrap().detach().unwrap());
            }

            v.into_iter().for_each(|c| pool.put(c));
            assert_eq!(pool.size(), 5);
        };
        tokio_run_async!(fut);
    }

    #[test]
    fn initialize_10() {
        let pool = Pool::<TcpConn>::builder()
            .connector(|| futures::future::ok(TcpConn(true)))
            .capacity(None)
            .build();

        let fut = async move {
            pool.initialize(10).await.unwrap();
            assert_eq!(pool.size(), 10);
        };
        tokio_run_async!(fut);
    }

    #[test]
    fn initialize_10_capacity_8() {
        let pool = Pool::<TcpConn>::builder()
            .connector(|| futures::future::ok(TcpConn(true)))
            .capacity(Some(8))
            .build();

        let fut = async move {
            pool.initialize(10).await.unwrap();
            assert_eq!(pool.size(), 8);
        };
        tokio_run_async!(fut);
    }

    #[test]
    fn initialize_10_capacity_11() {
        let pool = Pool::<TcpConn>::builder()
            .connector(|| futures::future::ok(TcpConn(true)))
            .capacity(Some(11))
            .build();

        let fut = async move {
            pool.initialize(10).await.unwrap();
            assert_eq!(pool.size(), 10);
        };
        tokio_run_async!(fut);
    }

    #[test]
    fn initialize_10_destroy_3() {
        let pool = Pool::<TcpConn>::builder()
            .connector(|| futures::future::ok(TcpConn(true)))
            .build();

        let fut = async move {
            pool.initialize(10).await.unwrap();
            pool.destroy(3);
            assert_eq!(pool.size(), 7);
        };
        tokio_run_async!(fut);
    }

    #[test]
    fn initialize_10_destroy_11() {
        let pool = Pool::<TcpConn>::builder()
            .connector(|| futures::future::ok(TcpConn(true)))
            .build();

        let fut = async move {
            pool.initialize(10).await.unwrap();
            pool.destroy(11);
            assert_eq!(pool.size(), 0);
        };
        tokio_run_async!(fut);
    }

    #[test]
    fn constant_backoff_100ms() {
        use std::sync::{Arc,Mutex};

        let first_invoke = Arc::new(Mutex::new(None));
        let second_invoke = Arc::new(Mutex::new(None));

        let f = first_invoke.clone();
        let s = second_invoke.clone();

        let pool = Pool::<TcpConn>::builder()
            .connector(move || {
                let mut f = f.lock().unwrap();
                let mut s = s.lock().unwrap();


                if f.is_none() {
                    *f = Some(Instant::now());
                    futures::future::ok(TcpConn(false))
                } else {
                    *s = Some(Instant::now());
                    futures::future::ok(TcpConn(true))
                }
            })
            .backoff(BackoffStrategy::Fixed(FixedIntervalBackoff::from_millis(100)))
            .build();

        let fut = async move {
            pool.initialize(2).await.unwrap();
        };
        tokio_run_async!(fut);

        let f = first_invoke.lock().unwrap();
        let s = second_invoke.lock().unwrap();
        let diff = s.unwrap().duration_since(f.unwrap());
        assert!(diff.as_millis() >= 100);
    }

    #[test]
    fn fail_after_3_max_tries() {
        use std::sync::{Arc, Mutex};

        let c = Arc::new(Mutex::new(0));
        let cc = c.clone();

        let pool = Pool::<TcpConnErr>::builder()
            .connector(move || {
                let cc = cc.clone();
                let mut cc = cc.lock().unwrap();
                *cc = (*cc) + 1;
                futures::future::ok(TcpConnErr(ErrorKind::BrokenPipe))
            })
            .max_tries(Some(3))
            .build();

        let fut = async move {
            match pool.take().await {
                Ok(_) => panic!("should not work"),
                Err(err) => assert_eq!(err.kind(), ErrorKind::BrokenPipe),
            };
        };
        tokio_run_async!(fut);
        assert_eq!(3, *c.lock().unwrap());
    }

    #[derive(Debug, Clone)]
    struct TcpConnErr(ErrorKind);

    impl Connection for TcpConnErr {
        fn test_poll(&mut self, _: &mut Context) -> Poll<Result<bool>> {
            Poll::Ready(Err(Error::from(self.0)))
        }
    }
}
