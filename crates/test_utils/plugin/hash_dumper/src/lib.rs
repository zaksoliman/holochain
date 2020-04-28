use holochain_serialized_bytes::prelude::*;
use holo_hash_core::EntryHash;

#[no_mangle]
pub extern fn hash_dump(ptr: usize, len: usize) -> (usize, usize) {
    let hash: EntryHash = sb.try_into().unwrap();
    println!("{:?}", &hash);
    let sb: SerializedBytes = hash.try_into().unwrap();
    let bytes = UnsafeBytes::from(sb);

}
