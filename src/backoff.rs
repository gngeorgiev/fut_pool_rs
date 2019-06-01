pub use tokio_retry::strategy::{
    jitter, ExponentialBackoff, FibonacciBackoff, FixedInterval as FixedIntervalBackoff,
};

#[derive(Clone)]
pub enum BackoffStrategy {
    Exponential(ExponentialBackoff),
    Fibonacci(FibonacciBackoff),
    Fixed(FixedIntervalBackoff),
    None,
}
