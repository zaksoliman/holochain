use crate::query::entry_details::GetEntryDetailsQuery;
use contrafact::{arbitrary::Unstructured, *};
use element_details::GetElementDetailsQuery;
use holochain_keystore::test_keystore::spawn_test_keystore;
use holochain_types::{dht_op::facts as op_facts, prelude::*};

use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn entry_scratch_same_as_sql() {
    observability::test_run().ok();
    let keystore = spawn_test_keystore().await.unwrap();
    let mut scratch = Scratch::new();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    let mut u = Unstructured::new(&NOISE);

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let valid_store_op = move || {
        facts![
            op_facts::op_is_valid(keystore.clone()),
            op_facts::op_of_type(DhtOpType::StoreEntry),
        ]
    };

    let op = valid_store_op().build(&mut u);
    valid_store_op().check(&op).unwrap();
    todo!("Rewrite test in terms of facts");

    let td = EntryTestData::new();
    let query = GetEntryDetailsQuery::new(td.hash.clone());
    insert_op_scratch(&mut scratch, td.store_entry_op.clone()).unwrap();
    insert_op(&mut txn, td.store_entry_op.clone(), true).unwrap();
    set_validation_status(
        &mut txn,
        td.store_entry_op.as_hash().clone(),
        ValidationStatus::Valid,
    )
    .unwrap();
    let r1 = query
        .run(Txn::from(&txn))
        .unwrap()
        .expect("Element not found");
    let r2 = query
        .run(scratch.clone())
        .unwrap()
        .expect("Element not found");
    assert_eq!(r1, r2);
}

#[tokio::test(flavor = "multi_thread")]
async fn element_scratch_same_as_sql() {
    observability::test_run().ok();
    let mut scratch = Scratch::new();
    let mut conn = Connection::open_in_memory().unwrap();
    SCHEMA_CELL.initialize(&mut conn, None).unwrap();

    let mut txn = conn
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();

    let td = ElementTestData::new();
    let query = GetElementDetailsQuery::new(td.header.as_hash().clone());
    insert_op_scratch(&mut scratch, td.store_element_op.clone()).unwrap();
    insert_op(&mut txn, td.store_element_op.clone(), true).unwrap();
    set_validation_status(
        &mut txn,
        td.store_element_op.as_hash().clone(),
        ValidationStatus::Valid,
    )
    .unwrap();
    let r1 = query
        .run(Txn::from(&txn))
        .unwrap()
        .expect("Element not found");
    let r2 = query
        .run(scratch.clone())
        .unwrap()
        .expect("Element not found");
    assert_eq!(r1, r2);
}
