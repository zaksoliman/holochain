#[no_mangle]
pub extern fn hash_dump(sb: SerializedBytes) -> SerializedBytes {
    let hash: EntryHash = sb.try_into().unwrap();
    println!("{:?}", &hash);
    hash.try_into().unwrap()
}
