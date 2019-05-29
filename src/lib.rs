#![feature(async_await)]

mod builder;
mod connector;
mod connection;
mod pool;
mod taker;
mod guard;

mod tcp;

pub use builder::PoolBuilder;
pub use connection::Connection;
pub use pool::{Pool};
pub use guard::PoolGuard;
pub use taker::PoolTaker;

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::Context;
    use std::time::Duration;
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
