#![feature(async_await)]

#[macro_use]
extern crate log;

use std::net::SocketAddr;
use std::ops::Deref;
use std::io::{self, Read, Write};

use futures::compat::Future01CompatExt;
use futures::task::Context;
use futures::Poll;

use tokio::io::{Result, AsyncWrite, AsyncRead};
use tokio::net::TcpStream;

use bytes::{Buf, BufMut};

use fut_pool::{PoolObject};

pub struct TcpConnection(TcpStream);

impl TcpConnection {
    pub async fn connect(addr: SocketAddr) -> Result<Self> {
        let stream = TcpStream::connect(&addr).compat().await?;
        Ok(TcpConnection(stream))
    }
}

impl Deref for TcpConnection {
    type Target = TcpStream;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PoolObject for TcpConnection {
    fn test_poll(&mut self, _: &mut Context) -> Poll<Result<bool>> {
        let mut buf = [0, 1];
        match self.0.poll_peek(&mut buf) {
            Ok(futures01::Async::Ready(_)) | Ok(futures01::Async::NotReady) => {
                debug!("PoolObject is alive");
                Poll::Ready(Ok(true))
            },
            Err(err) => {
                debug!("PoolObject Err={}", &err);
                Poll::Ready(Err(err))
            },
        }
    }
}

// ===== impl Read / Write =====

impl Read for TcpConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for TcpConnection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsyncRead for TcpConnection {
    fn read_buf<B: BufMut>(&mut self, buf: &mut B) -> futures01::Poll<usize, io::Error> {
        self.0.read_buf(buf)
    }
}

impl AsyncWrite for TcpConnection {
    fn shutdown(&mut self) -> futures01::Poll<(), io::Error> {
        <&TcpStream>::shutdown(&mut &self.0)
    }

    fn write_buf<B: Buf>(&mut self, buf: &mut B) -> futures01::Poll<usize, io::Error> {
        self.0.write_buf(buf)
    }
}

// ===== impl Read / Write for &'a =====

impl<'a> Read for &'a TcpConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        <&TcpStream>::read(&mut &self.0, buf)
    }
}

impl<'a> Write for &'a TcpConnection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        <&TcpStream>::write(&mut &self.0, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        <&TcpStream>::flush(&mut &self.0)
    }
}

impl<'a> AsyncRead for &'a TcpConnection {
    fn read_buf<B: BufMut>(&mut self, buf: &mut B) -> futures01::Poll<usize, io::Error> {
        <&TcpStream>::read_buf(&mut &self.0, buf)
    }
}

impl<'a> AsyncWrite for &'a TcpConnection {
    fn shutdown(&mut self) -> futures01::Poll<(), io::Error> {
        Ok(().into())
    }

    fn write_buf<B: Buf>(&mut self, buf: &mut B) -> futures01::Poll<usize, io::Error> {
        <&TcpStream>::write_buf(&mut &self.0, buf)
    }
}

impl std::fmt::Debug for TcpConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures::compat::*;
    use futures::stream::StreamExt;
    use futures::channel::oneshot::{channel, Sender};

    use fut_pool::Pool;
    use std::net::SocketAddr;

    use tokio::io::{Result};
    use tokio::net::TcpListener;
    use tokio::prelude::*;
    use std::str;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    macro_rules! tokio_run_async {
        ($e:expr) => {
            use futures::{FutureExt, TryFutureExt};

            let mut rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on($e.unit_error().boxed().compat()).unwrap();
        };
    }

    macro_rules! tokio_spawn_async {
        ($e:expr) => {
                use futures::{FutureExt, TryFutureExt};

                tokio::spawn($e
                    .unit_error()
                    .boxed()
                    .compat()
                    .then(|_| Ok(()))
                )
            };
    }

    const MESSAGE: &'static [u8] = b"HAI";

    async fn tokio_server(sender: Sender<()>) -> Result<()> {
        let addr = "127.0.0.1:5000".parse().unwrap();
        let mut server = TcpListener::bind(&addr).unwrap().incoming().compat();
        sender.send(()).unwrap();
        for socket in server.next().await.unwrap() {
            let fut = async move {
                let socket: TcpStream = socket;
                let (read, write) = socket.split();
                tokio::io::copy(read, write).compat().await.expect("copy");
            };
            tokio_spawn_async!(fut);
        }

        Ok(())
    }

    #[test]
    fn tcp_conn_compiles() {
        init();

        let addr: SocketAddr = "127.0.0.1:80".parse().unwrap();
        Pool::builder()
            .factory(move || TcpConnection::connect(addr))
            .build();
    }

    #[test]
    fn tcp_conn_connects() {
        init();

        let fut = async move {
            let (sender, receiver) = channel();
            tokio_spawn_async!(tokio_server(sender));
            receiver.await.expect("receiver");

            let addr: SocketAddr = "127.0.0.1:5000".parse().expect("socket addr");
            let pool = Pool::<TcpConnection>::builder()
                .factory(move || {
                    dbg!("connecting");
                    TcpConnection::connect(addr)
                })
                .build();

            let conn_guard = pool.take().await.expect("take");
            let conn = &*conn_guard;
            tokio::io::write_all(conn, MESSAGE).compat().await.expect("write_all");
            let (_, b) = tokio::io::read_exact(conn, vec![0; MESSAGE.len()]).compat().await.expect("read_to_end");

            assert_eq!(b.len(), MESSAGE.len());
            assert_eq!(str::from_utf8(&b).unwrap(), str::from_utf8(MESSAGE).unwrap());
        };
        tokio_run_async!(fut);
    }
}
