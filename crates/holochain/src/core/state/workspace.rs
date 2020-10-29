//! Workspaces are a simple abstraction used to stage changes during Workflow
//! execution to be persisted later
//!
//! Every Workflow has an associated Workspace type.

use super::source_chain::SourceChainError;
use holochain_state::{buffer::BufferedStore, error::DatabaseError};
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum WorkspaceError {
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),

    #[error(transparent)]
    SourceChainError(#[from] SourceChainError),
}

#[allow(missing_docs)]
pub type WorkspaceResult<T> = Result<T, WorkspaceError>;

/// Defines a Workspace
pub trait Workspace: BufferedStore + Send {}
impl<T> Workspace for T where T: BufferedStore + Send {}

#[cfg(test)]
pub mod tests {

    use crate::core::state::workspace::WorkspaceResult;
    use holochain_state::{
        buffer::{BufferedStore, KvBufFresh},
        db::{GetDb, ELEMENT_VAULT_HEADERS, ELEMENT_VAULT_PUBLIC_ENTRIES},
        prelude::*,
        test_utils::{test_cell_env, DbString},
    };
    use holochain_types::{prelude::*, test_utils::fake_header_hash};

    pub struct TestWorkspace {
        one: KvBufFresh<HeaderHash, u32>,
        two: KvBufFresh<DbString, bool>,
    }

    impl TestWorkspace {
        pub fn new(env: EnvironmentRead) -> WorkspaceResult<Self> {
            Ok(Self {
                one: KvBufFresh::new(env.clone(), env.get_db(&*ELEMENT_VAULT_PUBLIC_ENTRIES)?),
                two: KvBufFresh::new(env.clone(), env.get_db(&*ELEMENT_VAULT_HEADERS)?),
            })
        }
    }

    impl BufferedStore for TestWorkspace {
        type Error = crate::core::state::workspace::WorkspaceError;

        fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
            self.one.flush_to_txn_ref(writer)?;
            self.two.flush_to_txn_ref(writer)?;
            Ok(())
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn workspace_sanity_check() -> anyhow::Result<()> {
        let test_env = test_cell_env();
        let arc = test_env.env();
        let addr1 = fake_header_hash(1);
        let addr2: DbString = "hi".into();
        {
            let mut workspace = TestWorkspace::new(arc.clone().into())?;
            assert_eq!(workspace.one.get(&addr1)?, None);

            workspace.one.put(addr1.clone(), 1).unwrap();
            workspace.two.put(addr2.clone(), true).unwrap();
            assert_eq!(workspace.one.get(&addr1)?, Some(1));
            assert_eq!(workspace.two.get(&addr2)?, Some(true));
            arc.guard()
                .with_commit(|mut writer| workspace.flush_to_txn(&mut writer))?;
        }

        // Ensure that the data was persisted
        {
            let workspace = TestWorkspace::new(arc.clone().into())?;
            assert_eq!(workspace.one.get(&addr1)?, Some(1));
        }
        Ok(())
    }
}
