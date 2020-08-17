use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::ops::{Deref, DerefMut};

pub struct Sendable<'lt, T: 'lt + Send + Sync>(*mut T, std::marker::PhantomData<&'lt ()>);
unsafe impl<T: Send + Sync> Send for Sendable<'_, T> {}

pub struct Erased<'lt, T: 'lt + Send + Sync>(Option<Mutex<Sendable<'lt, T>>>);

impl<'lt, T: 'lt + Send + Sync> Drop for Erased<'lt, T> {
    fn drop(&mut self) {
        let inner = self.0.take().unwrap();
        let inner = inner.into_inner();
        unsafe { std::ptr::drop_in_place(inner.0); }
    }
}

impl<'lt, T: 'lt + Send + Sync> Erased<'lt, T> {
    pub fn new(t: T) -> Self {
        let t = Box::into_raw(Box::new(t));
        Self(Some(Mutex::new(Sendable(t, std::marker::PhantomData))))
    }

    pub async unsafe fn access(&self) -> MutexGuard<'_, Sendable<'lt, T>> {
        self.0.as_ref().unwrap().lock().await
    }
}

pub struct OwnedReadGuard<'lt, T: 'lt + Send + Sync> {
    lock: Option<Erased<'lt, Arc<RwLock<T>>>>,
    guard: Option<Erased<'lt, RwLockReadGuard<'lt, T>>>,
}

impl<'lt, T: 'lt + Send + Sync> Drop for OwnedReadGuard<'lt, T> {
    fn drop(&mut self) {
        // order maters
        drop(self.guard.take());
        drop(self.lock.take());
    }
}

impl<T: 'static + Send + Sync> OwnedReadGuard<'static, T> {
    pub async fn new(lock: Arc<RwLock<T>>) -> Self {
        let lock = Erased::new(lock);
        let guard = unsafe { lock.access().await.0.as_ref().unwrap().read() };
        let guard = Erased::new(guard.await);
        Self {
            lock: Some(lock),
            guard: Some(guard),
        }
    }
}

impl<'lt, T: 'lt + Send + Sync> OwnedReadGuard<'lt, T> {
    pub async fn read(&self) -> &T {
        unsafe { self.guard.as_ref().unwrap().access().await.deref().0.as_ref().unwrap() }
    }
}

pub struct OwnedWriteGuard<'lt, T: 'lt + Send + Sync> {
    lock: Option<Erased<'lt, Arc<RwLock<T>>>>,
    guard: Option<Erased<'lt, RwLockWriteGuard<'lt, T>>>,
}

impl<'lt, T: 'lt + Send + Sync> Drop for OwnedWriteGuard<'lt, T> {
    fn drop(&mut self) {
        // order maters
        drop(self.guard.take());
        drop(self.lock.take());
    }
}

impl<T: 'static + Send + Sync> OwnedWriteGuard<'static, T> {
    pub async fn new(lock: Arc<RwLock<T>>) -> Self {
        let lock = Erased::new(lock);
        let guard = unsafe { lock.access().await.0.as_ref().unwrap().write() };
        let guard = Erased::new(guard.await);
        Self {
            lock: Some(lock),
            guard: Some(guard),
        }
    }
}

impl<'lt, T: 'lt + Send + Sync> OwnedWriteGuard<'lt, T> {
    pub async fn read(&self) -> &T {
        unsafe { self.guard.as_ref().unwrap().access().await.deref().0.as_ref().unwrap() }
    }

    pub async fn write(&mut self) -> &mut T {
        unsafe { self.guard.as_ref().unwrap().access().await.deref_mut().0.as_mut().unwrap() }
    }
}
