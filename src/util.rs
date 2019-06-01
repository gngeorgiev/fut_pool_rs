#[macro_export]
macro_rules! poll_future_01_in_03 {
    ($e:expr) => {
        match $e {
            Ok(futures01::Async::Ready(b)) => b,
            Ok(futures01::Async::NotReady) => return Poll::Pending,
            Err(err) => return Poll::Ready(Err(err)),
        }
    };
}
