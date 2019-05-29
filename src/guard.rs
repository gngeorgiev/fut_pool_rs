use crate::connection::Connection;
use crate::pool::Pool;

pub struct PoolGuard<T>
where
    T: Connection,
{
    connection: Option<T>,
    pool: Pool<T>,
}

impl<T> PoolGuard<T>
where
    T: Connection,
{
    pub(crate) fn new(connection: T, pool: Pool<T>) -> PoolGuard<T> {
        PoolGuard {
            connection: Some(connection),
            pool,
        }
    }
}

impl<T> PoolGuard<T>
where
    T: Connection,
{
    pub fn detach(&mut self) -> Option<T> {
        let connection = self.connection.take();
        connection
    }
}

impl<T> std::ops::Deref for PoolGuard<T>
where
    T: Connection,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.connection.as_ref().expect("deref PoolGuard no inner")
    }
}

impl<T> Drop for PoolGuard<T>
where
    T: Connection,
{
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            self.pool.put(connection);
        }
    }
}
