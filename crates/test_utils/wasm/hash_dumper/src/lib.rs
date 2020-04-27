extern crate wee_alloc;

use holo_hash_core::EntryHash;
use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::plugin::PluginCall;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// define the host functions we require in order to pull/push data across the host/guest boundary
memory_externs!();

host_externs!(__plugin);

#[no_mangle]
pub extern "C" fn hash_dump(_: RemotePtr) -> RemotePtr {
 let entry_hash = EntryHash::new(vec![0xdb; 36]);

 let output: PluginOutput = try_result!(
     host_call!(__plugin, PluginInput::new(PluginCall{
         lib: "hash_dumper".to_string(),
         func: "hash_dump".to_string(),
         payload: SerializedBytes::try_from(&entry_hash).unwrap(),
     })),
     "failed to plugin"
 );

 let output_sb: SerializedBytes = try_result!(
     output.try_into(),
     "failed to serialize output for extern response"
 );
 ret!(ZomeExternGuestOutput::new(output_sb));
}
