use futures::Future;
use std::pin::Pin;
use tokio::io::Result;

pub type Connector<T> = Fn() -> Pin<Box<Future<Output = Result<T>>>> + 'static + Send + Sync;
