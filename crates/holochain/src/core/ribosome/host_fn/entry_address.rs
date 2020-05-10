use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use holochain_zome_types::EntryAddressInput;
use holochain_zome_types::EntryAddressOutput;
use std::sync::Arc;

pub async fn entry_address(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: EntryAddressInput,
) -> RibosomeResult<EntryAddressOutput> {
    unimplemented!();
}
