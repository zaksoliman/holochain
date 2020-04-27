use super::HostContext;
use super::WasmRibosome;
use holochain_zome_types::PluginInput;
use holochain_zome_types::PluginOutput;
use std::sync::Arc;
use std::convert::TryInto;

pub async fn plugin(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: PluginInput,
) -> PluginOutput {
    println!("fooo");
    PluginOutput::new(().try_into().unwrap())
}

#[cfg(test)]
pub mod wasm_test {
    use holochain_zome_types::{PluginOutput};

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_plugin_test() {
        let _: PluginOutput =
            crate::call_test_ribosome!("hash_dumper", "hash_dump", ());
    }
}
