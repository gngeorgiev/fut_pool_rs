use crate::object::PoolObject;
use crate::pool::Pool;

pub struct PoolGuard<T>
where
    T: PoolObject,
{
    object: Option<T>,
    pool: Pool<T>,
}

impl<T> PoolGuard<T>
where
    T: PoolObject,
{
    pub(crate) fn new(object: T, pool: Pool<T>) -> PoolGuard<T> {
        PoolGuard {
            object: Some(object),
            pool,
        }
    }
}

impl<T> PoolGuard<T>
where
    T: PoolObject,
{
    pub fn detach(&mut self) -> Option<T> {
        let object = self.object.take();
        object
    }
}

impl<T> std::ops::Deref for PoolGuard<T>
where
    T: PoolObject,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.object.as_ref().expect("deref PoolGuard no inner")
    }
}

impl<T> std::ops::DerefMut for PoolGuard<T>
where
    T: PoolObject,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.object.as_mut().expect("deref PoolGuard no inner")
    }
}

impl<T> Drop for PoolGuard<T>
where
    T: PoolObject,
{
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            self.pool.put(object);
        }
    }
}
