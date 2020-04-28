use super::HostContext;
use super::WasmRibosome;
use holochain_zome_types::PluginInput;
use holochain_zome_types::PluginOutput;
use std::sync::Arc;
use std::path::Path;
use sharedlib::LibArc;
use sharedlib::FuncArc;
use holochain_serialized_bytes::prelude::*;
use sharedlib::Symbol;
use holo_hash::holo_hash_core::EntryHash;

pub async fn plugin(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: PluginInput,
) -> PluginOutput {
    let target_dir = std::env::var_os("CARGO_TARGET_DIR").unwrap();
    println!("fooo {:?}", std::process::Command::new("pwd").output().unwrap());
    let lib_path = Path::new(&target_dir).join("release/libtest_plugin_hash_dumper.so");
    let hash_dump = unsafe {
        let lib = LibArc::new(lib_path).unwrap();
        let hash_dump_symbol: FuncArc<extern "C" fn(SerializedBytes) -> SerializedBytes> = lib.find_func("hash_dump").unwrap();
        hash_dump_symbol.get()
    };

    let entry_hash = EntryHash::new(vec![0xdb; 36]);
    let entry_hash_sb = SerializedBytes::try_from(entry_hash).unwrap();
    hash_dump(entry_hash_sb);
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
