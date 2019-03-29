use futures::Future;
use std::io;

pub type Connector<T> = Fn() -> Box<Future<Item = T, Error = io::Error>> + Send + Sync + 'static;
