mod builder;
mod connector;
mod pool;

pub use builder::PoolBuilder;
pub use connector::Connector;
pub use pool::{Pool, PoolTaker, Peek};

#[cfg(test)]
mod tests {
    use super::*;
    use futures::prelude::*;
    use futures::{Poll, Async};
    use tokio::io::{Error, ErrorKind};
    use std::sync::Arc;

    #[derive(Debug, Clone)]
    struct TcpConn;

    impl Peek for TcpConn {
        fn poll_peek(&mut self, _buf: &mut [u8]) -> Poll<usize, Error> {
            Ok(Async::Ready(1))
        }
    }

    #[test]
    fn default_values() {
        let pool = Pool::builder()
            .connector(Box::new(|| Box::new(futures::future::ok(TcpConn))))
            .build();

        assert_eq!(0, pool.size());
        assert_eq!(true, pool.try_take().is_none());
    }

    #[test]
    fn connect_1() {
        let pool = Pool::builder()
            .connector(Box::new(|| Box::new(futures::future::ok(TcpConn))))
            .build();

        let p = pool.clone();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(pool.take().and_then(move |s| {
                p.put(s);
                Ok(())
            }))
            .unwrap();

        assert_eq!(1, pool.size());
        assert_eq!(true, pool.try_take().is_some());
    }

    #[test]
    fn connect_2_take_detach_one() {
        let pool = Pool::builder()
            .connector(Box::new(|| Box::new(futures::future::ok(TcpConn))))
            .build();


        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let p = pool.clone();
        rt.block_on(pool.take().and_then(move |s| {
            p.put(s.clone());
            p.put(s.clone());
            Ok(())
        })).unwrap();

        assert_eq!(2, pool.size());
        let conn = pool.try_take();
        assert_eq!(true, conn.is_some());
        pool.put(conn.unwrap());

        let item = pool.try_take().unwrap();
        assert_eq!(1, pool.size());
        pool.put(item);
        assert_eq!(2, pool.size());

        let item = pool.try_take().unwrap();
        assert_eq!(1, pool.size());
        drop(item);
        assert_eq!(1, pool.size());
    }

    #[derive(Debug, Clone)]
    struct TcpConnErrPeek;

    impl Peek for TcpConnErrPeek {
        fn poll_peek(&mut self, _buf: &mut [u8]) -> Poll<usize, Error> {
            Err(Error::from(ErrorKind::BrokenPipe))
        }
    }
}
