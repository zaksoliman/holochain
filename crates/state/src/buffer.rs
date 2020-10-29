#![allow(missing_docs)]
use crate::{
    error::{DatabaseError, DatabaseResult},
    transaction::Writer,
};

mod cas;
pub mod iter;
mod kv;
mod kvv;

pub use cas::{CasBufFreshAsync, CasBufFreshSync};
pub use kv::{KvBufFresh, KvBufUsed, KvIntBufFresh, KvIntBufUsed, KvIntStore, KvStore, KvStoreT};
pub use kvv::KvvBufUsed;

// Empty keys break lmdb
pub(super) fn check_empty_key<K: AsRef<[u8]>>(k: &K) -> DatabaseResult<()> {
    if k.as_ref().is_empty() {
        Err(DatabaseError::EmptyKey)
    } else {
        Ok(())
    }
}

/// General trait for transactional stores, exposing only the method which
/// adds changes to the write transaction. This generalization is not really used,
/// but could be used in Workspaces i.e. iterating over a Vec<dyn BufferedStore>
/// is all that needs to happen to commit the workspace changes
pub trait BufferedStore: Sized {
    /// The error type for `flush_to_txn` errors
    type Error: std::error::Error;

    /// Flush accumulated changes in the scratch space to the `writer`,
    /// without committing.
    ///
    /// No method is provided to commit the writer as well, because Writers
    /// should be managed such that write failures are properly handled, which
    /// is outside the scope of the workspace.
    ///
    /// This method is provided and shouldn't need to be implemented. It is
    /// preferred to use this over `flush_to_txn_ref` since it's generally not
    /// valid to flush the same data twice.
    fn flush_to_txn(mut self, writer: &mut Writer) -> Result<(), Self::Error> {
        self.flush_to_txn_ref(writer)
    }

    /// Flush changes to the `writer` without consuming self. `flush_to_txn`
    /// is preferred over this whenever possible
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> Result<(), Self::Error>;

    /// Specifies whether there are actually changes to flush. If not, the
    /// flush_to_txn method may decide to do nothing.
    fn is_clean(&self) -> bool {
        false
    }
}

#[macro_export]
/// Macro to generate a fresh reader from an EnvironmentRead with less boilerplate
macro_rules! fresh_reader {
    ($env: expr, $f: expr) => {{
        let g = $env.guard();
        let r = $crate::env::ReadManager::reader(&g)?;
        $f(r)
    }};
}

#[macro_export]
/// Macro to generate a fresh reader from an EnvironmentRead with less boilerplate
/// Use this in tests, where everything gets unwrapped anyway
macro_rules! fresh_reader_test {
    ($env: expr, $f: expr) => {{
        let g = $env.guard();
        let r = $crate::env::ReadManager::reader(&g).unwrap();
        $f(r)
    }};
}
