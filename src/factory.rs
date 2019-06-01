use std::pin::Pin;

use futures::Future;
use std::io::Result;

pub type ObjectFactory<T> =
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<T>>>> + 'static + Send + Sync;
