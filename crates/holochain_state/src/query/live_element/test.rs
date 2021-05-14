use contrafact::arbitrary::Arbitrary;
use contrafact::arbitrary::Unstructured;
use holochain_keystore::test_keystore::spawn_test_keystore;
use holochain_sqlite::rusqlite::Connection;
use holochain_sqlite::rusqlite::TransactionBehavior;
use holochain_sqlite::schema::SCHEMA_CELL;
use holochain_types::dht_op::facts::valid_op_with_header_and_entry;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::header::facts::update_with_entry_and_valid_author;

use crate::mutations::insert_op_scratch;
use crate::mutations::set_validation_status;
use crate::prelude::mutations_helpers::insert_valid_authored_op;
use contrafact::*;

use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn can_handle_update_in_scratch() {
    observability::test_run().ok();
    let mut scratch = Scratch::new();
    let mut conn = Connection::open_in_memory().unwrap();
    let keystore = spawn_test_keystore().await.unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let mut u = Unstructured::new(&NOISE);
    let entry = Entry::arbitrary(&mut u).unwrap();
    let update_header: Header =
        update_with_entry_and_valid_author(entry.clone(), keystore.clone()).build(&mut u);
    let update_hash = update_header.to_hash();
    let update_store_element_op: DhtOpHashed = valid_op_with_header_and_entry(
        keystore.clone(),
        DhtOpType::StoreElement,
        update_header.clone(),
        Some(entry.clone()),
    )
    .build(&mut u)
    .into_hashed();

    let query = GetLiveElementQuery::new(update_hash);

    // - Create an entry on main db.
    insert_valid_authored_op(&mut txn, update_store_element_op.clone()).unwrap();
    set_validation_status(
        &mut txn,
        update_store_element_op.as_hash().clone(),
        ValidationStatus::Valid,
    )
    .unwrap();
    let r = query
        .run(Txn::from(&txn))
        .unwrap()
        .expect("Element not found");
    assert_eq!(*r.entry().as_option().unwrap(), entry);
    assert_eq!(*r.header(), update_header);

    // - Add to the scratch
    insert_op_scratch(&mut scratch, update_store_element_op.clone()).unwrap();
    let r = query
        .run(scratch.clone())
        .unwrap()
        .expect("Element not found");
    assert_eq!(*r.entry().as_option().unwrap(), entry);
    assert_eq!(*r.header(), update_header);
}
