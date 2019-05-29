use futures::task::Context;
use futures::Poll;

use tokio::io::Result;
use tokio::net::TcpStream;

use crate::connection::Connection;

impl Connection for TcpStream {
    fn test_poll(&mut self, _: &mut Context) -> Poll<Result<bool>> {
        let mut buf = [0, 1];
        match self.poll_peek(&mut buf) {
            Ok(futures01::Async::Ready(b)) => Poll::Ready(Ok(b > 0)),
            Ok(futures01::Async::NotReady) => Poll::Pending,
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::compat::Future01CompatExt;

    use crate::pool::Pool;
    use std::net::SocketAddr;
    use tokio::net::TcpStream;

    #[test]
    fn tcp_stream_compiles() {
        let addr: SocketAddr = "127.0.0.1:80".parse().unwrap();
        Pool::builder()
            .connector(move || TcpStream::connect(&addr).compat())
            .build();
    }
}
