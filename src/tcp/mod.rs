use futures::task::Context;
use futures::Poll;

use tokio::io::Result;
use tokio::net::TcpStream;

use crate::connection::Connection;

impl Connection for TcpStream {
    fn test_poll(&mut self, _: &mut Context) -> Poll<Result<bool>> {
        let mut buf = [0, 1];
        let b = poll_future_01_in_03!(self.poll_peek(&mut buf));
        Poll::Ready(Ok(b > 0))
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
            .factory(move || TcpStream::connect(&addr).compat())
            .build();
    }
}
