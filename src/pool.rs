use std::collections::VecDeque;
use std::io::Result;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;

use crate::backoff::BackoffStrategy;
use crate::builder::PoolBuilder;
use crate::factory::ObjectFactory;
use crate::guard::PoolGuard;
use crate::object::PoolObject;
use crate::taker::PoolTaker;

pub struct Pool<T>
where
    T: PoolObject,
{
    pub(crate) factory: Arc<ObjectFactory<T>>,
    pub(crate) objects: Arc<RwLock<VecDeque<T>>>,

    pub(crate) timeout: Option<Duration>,
    pub(crate) max_tries: Option<usize>,
    pub(crate) capacity: Option<usize>,
    pub(crate) backoff: BackoffStrategy,
}

impl<T> Clone for Pool<T>
where
    T: PoolObject,
{
    fn clone(&self) -> Self {
        Pool {
            factory: self.factory.clone(),
            backoff: self.backoff.clone(),
            objects: self.objects.clone(),
            timeout: self.timeout.clone(),
            max_tries: self.max_tries.clone(),
            capacity: self.capacity.clone(),
        }
    }
}

impl<T> Pool<T>
where
    T: PoolObject,
{
    pub fn builder() -> PoolBuilder<T> {
        PoolBuilder::new()
    }

    pub async fn take(&self) -> Result<PoolGuard<T>> {
        PoolTaker::<T>::new(self.clone()).await
    }

    pub fn try_take(&self) -> Option<PoolGuard<T>> {
        let mut connections = self.objects.write();
        let conn = connections.pop_front()?;
        Some(PoolGuard::new(conn, self.clone()))
    }

    pub fn put(&self, obj: T) {
        let mut objects = self.objects.write();
        let capacity = self.capacity.unwrap_or_else(|| 0);
        if capacity > 0 && objects.len() >= capacity {
            objects.pop_back();
        }

        objects.push_back(obj);
    }

    pub fn size(&self) -> usize {
        self.objects.read().len()
    }

    pub async fn initialize(&self, amount: usize) -> Result<()> {
        let amount = self
            .capacity
            .map(|cap| if cap > 0 && amount > cap { cap } else { amount })
            .unwrap_or(amount);

        let mut initialized = Vec::with_capacity(amount);
        for _ in 0..amount {
            initialized.push(self.take().await?.detach().unwrap());
        }
        initialized.into_iter().for_each(|c| self.put(c));

        Ok(())
    }

    pub fn destroy(&self, mut amount: usize) {
        loop {
            if amount == 0 {
                break;
            }

            if let Some(mut item) = self.try_take() {
                amount -= 1;
                item.detach();
            } else {
                break;
            }
        }
    }
}
