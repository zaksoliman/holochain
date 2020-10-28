#![allow(clippy::mutex_atomic)]
use super::*;
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

#[derive(Clone, shrinkwraprs::Shrinkwrap)]
pub struct CallZomeWorkspaceLock(Arc<Mutex<CallZomeWorkspace>>);

impl CallZomeWorkspaceLock {
    pub fn new(workspace: CallZomeWorkspace) -> Self {
        Self(Arc::new(Mutex::new(workspace)))
    }

    pub fn into_inner(self) -> Arc<Mutex<CallZomeWorkspace>> {
        self.0
    }

    #[tracing::instrument(skip(self))]
    pub async fn read<'a>(&'a self) -> CallZomeWorkspaceLockReadGuard<'a> {
        tracing::info!("acquiring lock");
        let g = CallZomeWorkspaceLockReadGuard(self.0.lock().await);
        tracing::info!("lock acquired");
        g
    }

    #[tracing::instrument(skip(self))]
    pub async fn write<'a>(&'a self) -> CallZomeWorkspaceLockWriteGuard<'a> {
        tracing::info!("acquiring lock");
        let g = CallZomeWorkspaceLockWriteGuard(self.0.lock().await);
        tracing::info!("lock acquired");
        g
    }
}

impl From<CallZomeWorkspace> for CallZomeWorkspaceLock {
    fn from(w: CallZomeWorkspace) -> Self {
        Self::new(w)
    }
}

#[derive(shrinkwraprs::Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct CallZomeWorkspaceLockReadGuard<'a>(MutexGuard<'a, CallZomeWorkspace>);

impl<'a> Drop for CallZomeWorkspaceLockReadGuard<'a> {
    #[tracing::instrument(skip(self))]
    fn drop(&mut self) {
        tracing::info!("lock dropped");
    }
}

#[derive(shrinkwraprs::Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct CallZomeWorkspaceLockWriteGuard<'a>(MutexGuard<'a, CallZomeWorkspace>);

impl<'a> Drop for CallZomeWorkspaceLockWriteGuard<'a> {
    #[tracing::instrument(skip(self))]
    fn drop(&mut self) {
        tracing::info!("lock dropped");
    }
}
