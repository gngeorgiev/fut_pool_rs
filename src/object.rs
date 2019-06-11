use futures::task::Context;
use futures::Poll;
use tokio::io::Result;

pub trait PoolObject {
    fn test_poll(&mut self, cx: &mut Context) -> Poll<Result<bool>>;
}
