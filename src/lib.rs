mod builder;
mod connector;
mod pool;

pub use builder::PoolBuilder;
pub use connector::Connector;
pub use pool::{Pool, PoolTaker};

#[cfg(test)]
mod tests {
    use super::*;
    use futures;
    use tokio;

    #[derive(Debug)]
    struct TcpConn;

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

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(pool.take())
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
        rt.block_on(futures::future::join_all(vec![pool.take(), pool.take()]))
            .unwrap();

        assert_eq!(2, pool.size());
        assert_eq!(true, pool.try_take().is_some());

        let item = pool.try_take().unwrap();
        assert_eq!(1, pool.size());
        drop(item);
        assert_eq!(2, pool.size());

        let mut item = pool.try_take().unwrap();
        assert_eq!(1, pool.size());
        item.detach();
        drop(item);
        assert_eq!(1, pool.size());
    }
}
