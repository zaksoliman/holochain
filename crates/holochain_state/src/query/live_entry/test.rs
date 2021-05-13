use contrafact::arbitrary::Arbitrary;
use contrafact::arbitrary::Unstructured;
use holochain_keystore::test_keystore::spawn_test_keystore;
use holochain_sqlite::rusqlite::Connection;
use holochain_sqlite::rusqlite::TransactionBehavior;
use holochain_sqlite::schema::SCHEMA_CELL;
use holochain_types::dht_op;
use holochain_types::dht_op::DhtOpHashed;

use crate::mutations::insert_op_scratch;
use crate::mutations::set_validation_status;
use crate::prelude::mutations_helpers::insert_valid_authored_op;
use crate::query::test_data::EntryTestData;
use contrafact::*;

use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn can_handle_update_in_scratch() {
    observability::test_run().ok();
    let keystore = spawn_test_keystore().await.unwrap();
    let mut scratch = Scratch::new();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();
    let mut u = Unstructured::new(&NOISE);

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    // let td = EntryTestData::new();
    let entry = Entry::arbitrary(&mut u).unwrap();
    let entry_hash = EntryHash::with_data_sync(&entry);
    let mut update_header = facts![
        header::facts::is_of_type(HeaderType::Update),
        header::facts::header_for_entry(entry.clone())
    ];
    let update_header: Header = update_header.build(&mut u);
    let mut valid_store_op = facts![
        dht_op::facts::op_is_valid(keystore.clone()),
        dht_op::facts::op_of_type(DhtOpType::StoreEntry),
        dht_op::facts::op_for_header(update_header.clone()),
        dht_op::facts::op_for_entry(entry.clone()),
    ];

    let update_store_entry_op: DhtOpHashed = valid_store_op.build(&mut u).into_hashed();
    let query = GetLiveEntryQuery::new(entry_hash.clone());

    // - Create an entry on main db.
    insert_valid_authored_op(&mut txn, update_store_entry_op.clone()).unwrap();
    set_validation_status(
        &mut txn,
        update_store_entry_op.as_hash().clone(),
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
    insert_op_scratch(&mut scratch, update_store_entry_op.clone()).unwrap();
    let r = query
        .run(scratch.clone())
        .unwrap()
        .expect("Element not found");
    assert_eq!(*r.entry().as_option().unwrap(), entry);
    assert_eq!(*r.header(), update_header);
}
