use crate::ribosome::{error::RibosomeResult, ZomeCallInvocation, ZomeCallInvocationResult};
use holochain_state::workspace::call_zome::CallZomeWorkspaceLock;
use holochain_zome_types::cell::CellId;
use std::sync::Arc;

#[async_trait::async_trait]
/// A handle that can only call zome functions to avoid
/// making write lock calls
pub trait CellConductorReadHandleT: Send + Sync {
    /// Get this cell id
    fn cell_id(&self) -> &CellId;
    /// Invoke a zome function on a Cell
    async fn call_zome(
        &self,
        invocation: ZomeCallInvocation,
        workspace_lock: &CallZomeWorkspaceLock,
    ) -> RibosomeResult<ZomeCallInvocationResult>;
}

/// A handle that cn only call zome functions to avoid
/// making write lock calls
pub type CellConductorReadHandle = Arc<dyn CellConductorReadHandleT>;
