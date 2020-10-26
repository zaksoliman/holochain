#![allow(clippy::mutex_atomic)]
use super::*;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Clone, shrinkwraprs::Shrinkwrap)]
pub struct CallZomeWorkspaceLock(Arc<RwLock<CallZomeWorkspace>>);

impl CallZomeWorkspaceLock {
    pub fn new(workspace: CallZomeWorkspace) -> Self {
        Self(Arc::new(RwLock::new(workspace)))
    }

    pub fn into_inner(self) -> Arc<RwLock<CallZomeWorkspace>> {
        self.0
    }

    #[tracing::instrument(skip(self))]
    pub async fn read<'a>(&'a self) -> CallZomeWorkspaceLockReadGuard<'a> {
        tracing::info!("read start");
        CallZomeWorkspaceLockReadGuard(self.0.read().await)
    }

    #[tracing::instrument(skip(self))]
    pub async fn write<'a>(&'a self) -> CallZomeWorkspaceLockWriteGuard<'a> {
        tracing::info!("write start");
        CallZomeWorkspaceLockWriteGuard(self.0.write().await)
    }
}

impl From<CallZomeWorkspace> for CallZomeWorkspaceLock {
    fn from(w: CallZomeWorkspace) -> Self {
        Self::new(w)
    }
}

#[derive(shrinkwraprs::Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct CallZomeWorkspaceLockReadGuard<'a>(RwLockReadGuard<'a, CallZomeWorkspace>);

impl<'a> Drop for CallZomeWorkspaceLockReadGuard<'a> {
    #[tracing::instrument(skip(self))]
    fn drop(&mut self) {
        tracing::info!("read drop");
    }
}

#[derive(shrinkwraprs::Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct CallZomeWorkspaceLockWriteGuard<'a>(RwLockWriteGuard<'a, CallZomeWorkspace>);

impl<'a> Drop for CallZomeWorkspaceLockWriteGuard<'a> {
    #[tracing::instrument(skip(self))]
    fn drop(&mut self) {
        tracing::info!("write drop");
    }
}
