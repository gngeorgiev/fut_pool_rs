use futures::future::FutureResult;
use futures::Future;
use std::io;

pub type Connector<T> = Fn() -> Box<Future<Item = T, Error = io::Error>> + Send + Sync + 'static;

//pub trait Connector<T> {
//    fn connect(&self) -> Box<Future<Item = T, Error = io::Error>>;
//}
//
//impl <T, F> Connector<T> for F where F: Fn() -> Box<Future<Item = T, Error = io::Error>> + Send + Sync + 'static {
//    fn connect(&self) -> Box<Future<Item = T, Error = io::Error>> {
//        self()
//    }
//}
//
//pub trait ConnectorResult<T> {
//    fn to_box(&self, fut: impl Future<Item = T, Error = io::Error> + 'static) -> Box<Future<Item = T, Error = io::Error>>;
//}
//
//impl <T, F> ConnectorResult<T> for F where F: Future<Item = T, Error = io::Error> {
//    fn to_box(&self, fut: impl Future<Item = T, Error = io::Error> + 'static) -> Box<Future<Item = T, Error = io::Error>> {
//        Box::new(fut)
//    }
//}
