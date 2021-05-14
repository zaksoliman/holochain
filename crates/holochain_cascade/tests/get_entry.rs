use contrafact::arbitrary::Arbitrary;
use contrafact::arbitrary::Unstructured;
use contrafact::*;
use ghost_actor::dependencies::observability;
use holo_hash::HasHash;
use holo_hash::HashableContentExtSync;
use holochain_cascade::test_utils::*;
use holochain_cascade::Cascade;
use holochain_p2p::HolochainP2pCellT;
use holochain_p2p::MockHolochainP2pCellT;
use holochain_state::mutations::insert_op_scratch;
use holochain_state::prelude::fresh_reader_test;
use holochain_state::prelude::test_cache_env;
use holochain_state::prelude::test_cell_env;
use holochain_state::scratch::Scratch;
use holochain_types::dht_op;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::Details;
use holochain_zome_types::ElementDetails;
use holochain_zome_types::Entry;
use holochain_zome_types::EntryDetails;
use holochain_zome_types::EntryDhtStatus;
use holochain_zome_types::GetOptions;
use holochain_zome_types::Header;
use holochain_zome_types::ValidationStatus;
use holochain_zome_types::NOISE;

async fn assert_can_get<N: HolochainP2pCellT + Clone + Send + 'static>(
    td_entry: &EntryTestData,
    td_element: &ElementTestData,
    cascade: &mut Cascade<N>,
    options: GetOptions,
) {
    // - Get via entry hash
    let r = cascade
        .dht_get(td_entry.hash.clone().into(), options.clone())
        .await
        .unwrap()
        .expect("Failed to get entry");

    assert_eq!(*r.header_address(), td_entry.create_hash);
    assert_eq!(r.header().entry_hash(), Some(&td_entry.hash));

    // - Get via header hash
    let r = cascade
        .dht_get(td_element.any_header_hash.clone().into(), options.clone())
        .await
        .unwrap()
        .expect("Failed to get element");

    assert_eq!(*r.header_address(), td_element.any_header_hash);
    assert_eq!(r.header().entry_hash(), td_element.any_entry_hash.as_ref());

    // - Get details via entry hash
    let r = cascade
        .get_details(td_entry.hash.clone().into(), options.clone())
        .await
        .unwrap()
        .expect("Failed to get entry");

    let expected = Details::Entry(EntryDetails {
        entry: td_entry.entry.entry.clone(),
        headers: vec![td_entry
            .wire_create
            .data
            .clone()
            .into_header(td_entry.entry.entry_type.clone(), td_entry.hash.clone())],
        rejected_headers: vec![],
        deletes: vec![],
        updates: vec![],
        entry_dht_status: EntryDhtStatus::Live,
    });

    assert_eq!(r, expected);

    // - Get details via header hash
    let r = cascade
        .get_details(td_element.any_header_hash.clone().into(), options.clone())
        .await
        .unwrap()
        .expect("Failed to get element details");

    let expected = Details::Element(ElementDetails {
        element: td_element.any_element.clone(),
        validation_status: ValidationStatus::Valid,
        deletes: vec![],
        updates: vec![],
    });
    assert_eq!(r, expected);
}

async fn assert_is_none<N: HolochainP2pCellT + Clone + Send + 'static>(
    td_entry: &EntryTestData,
    td_element: &ElementTestData,
    cascade: &mut Cascade<N>,
    options: GetOptions,
) {
    // - Get via entry hash
    let r = cascade
        .dht_get(td_entry.hash.clone().into(), options.clone())
        .await
        .unwrap();

    assert!(r.is_none());

    // - Get via header hash
    let r = cascade
        .dht_get(td_element.any_header_hash.clone().into(), options.clone())
        .await
        .unwrap();

    assert!(r.is_none());

    // - Get details via entry hash
    let r = cascade
        .get_details(td_entry.hash.clone().into(), options.clone())
        .await
        .unwrap();

    assert!(r.is_none());

    // - Get details via header hash
    let r = cascade
        .get_details(td_element.any_header_hash.clone().into(), options.clone())
        .await
        .unwrap();

    assert!(r.is_none());
}

async fn assert_rejected<N: HolochainP2pCellT + Clone + Send + 'static>(
    td_entry: &EntryTestData,
    td_element: &ElementTestData,
    cascade: &mut Cascade<N>,
    options: GetOptions,
) {
    // - Get via entry hash
    let r = cascade
        .dht_get(td_entry.hash.clone().into(), options.clone())
        .await
        .unwrap();

    assert!(r.is_none());

    // - Get via header hash
    let r = cascade
        .dht_get(td_element.any_header_hash.clone().into(), options.clone())
        .await
        .unwrap();

    assert!(r.is_none());

    let r = cascade
        .get_details(td_entry.hash.clone().into(), Default::default())
        .await
        .unwrap()
        .expect("Failed to get entry");

    let expected = Details::Entry(EntryDetails {
        entry: td_entry.entry.entry.clone(),
        headers: vec![],
        rejected_headers: vec![td_entry
            .wire_create
            .data
            .clone()
            .into_header(td_entry.entry.entry_type.clone(), td_entry.hash.clone())],
        deletes: vec![],
        updates: vec![],
        entry_dht_status: EntryDhtStatus::Dead,
    });

    assert_eq!(r, expected);

    let r = cascade
        .get_details(
            td_element.any_header_hash.clone().into(),
            Default::default(),
        )
        .await
        .unwrap()
        .expect("Failed to get entry");

    let expected = Details::Element(ElementDetails {
        element: td_element.any_element.clone(),
        validation_status: ValidationStatus::Rejected,
        deletes: vec![],
        updates: vec![],
    });

    assert_eq!(r, expected);
}

async fn assert_can_retrieve<N: HolochainP2pCellT + Clone + Send + 'static>(
    td_entry: &EntryTestData,
    cascade: &mut Cascade<N>,
    options: GetOptions,
) {
    // - Retrieve via entry hash
    let r = cascade
        .retrieve(td_entry.hash.clone().into(), options.clone().into())
        .await
        .unwrap()
        .expect("Failed to retrieve element");

    assert_eq!(*r.header_address(), td_entry.create_hash);
    assert_eq!(r.header().entry_hash(), Some(&td_entry.hash));

    // - Retrieve via header hash
    let r = cascade
        .retrieve(td_entry.create_hash.clone().into(), options.clone().into())
        .await
        .unwrap()
        .expect("Failed to retrieve element");

    assert_eq!(*r.header_address(), td_entry.create_hash);
    assert_eq!(r.header().entry_hash(), Some(&td_entry.hash));

    // - Retrieve entry
    let r = cascade
        .retrieve_entry(td_entry.hash.clone(), options.clone().into())
        .await
        .unwrap()
        .expect("Failed to retrieve entry");

    assert_eq!(*r.as_hash(), td_entry.hash);

    // - Retrieve header
    let r = cascade
        .retrieve_header(td_entry.create_hash.clone(), options.clone().into())
        .await
        .unwrap()
        .expect("Failed to retrieve header");

    assert_eq!(*r.as_hash(), td_entry.create_hash);
}

#[tokio::test(flavor = "multi_thread")]
async fn entry_not_authority_or_authoring() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let authority = test_cell_env();

    // Data
    let td_entry = EntryTestData::create();
    let td_element = ElementTestData::create();
    fill_db(&authority.env(), td_entry.store_entry_op.clone());
    fill_db(&authority.env(), td_element.any_store_element_op.clone());

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.env().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty().with_network(network, cache.env());

    assert_can_get(&td_entry, &td_element, &mut cascade, GetOptions::latest()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn entry_authoring() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let mut scratch = Scratch::new();

    // Data
    let td_entry = EntryTestData::create();
    let td_element = ElementTestData::create();
    insert_op_scratch(&mut scratch, td_entry.store_entry_op.clone()).unwrap();
    insert_op_scratch(&mut scratch, td_element.any_store_element_op.clone()).unwrap();

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pCellT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(false));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_scratch(scratch.into_sync())
        .with_network(mock, cache.env());

    assert_can_get(&td_entry, &td_element, &mut cascade, GetOptions::latest()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn entry_authority() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let vault = test_cell_env();

    // Data
    let td_entry = EntryTestData::create();
    let td_element = ElementTestData::create();
    fill_db(&vault.env(), td_entry.store_entry_op.clone());
    fill_db(&vault.env(), td_element.any_store_element_op.clone());

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pCellT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(true));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_vault(vault.env().into())
        .with_network(mock, cache.env());

    assert_can_get(&td_entry, &td_element, &mut cascade, GetOptions::latest()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn content_not_authority_or_authoring() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let vault = test_cell_env();

    // Data
    let td_entry = EntryTestData::create();
    let td_element = ElementTestData::create();
    fill_db(&vault.env(), td_entry.store_entry_op.clone());
    fill_db(&vault.env(), td_element.any_store_element_op.clone());

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pCellT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(false));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_vault(vault.env().into())
        .with_network(mock, cache.env());

    assert_can_get(&td_entry, &td_element, &mut cascade, GetOptions::content()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn content_authoring() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let mut scratch = Scratch::new();

    // Data
    let td_entry = EntryTestData::create();
    let td_element = ElementTestData::create();
    insert_op_scratch(&mut scratch, td_entry.store_entry_op.clone()).unwrap();
    insert_op_scratch(&mut scratch, td_element.any_store_element_op.clone()).unwrap();

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pCellT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(false));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_scratch(scratch.into_sync())
        .with_network(mock, cache.env());

    assert_can_get(&td_entry, &td_element, &mut cascade, GetOptions::content()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn content_authority() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let vault = test_cell_env();

    // Data
    let td_entry = EntryTestData::create();
    let td_element = ElementTestData::create();

    // Network
    // - Not expecting any calls to the network.
    let mut mock = MockHolochainP2pCellT::new();
    mock.expect_authority_for_hash().returning(|_| Ok(true));
    let mock = MockNetwork::new(mock);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_vault(vault.env().into())
        .with_network(mock, cache.env());

    assert_is_none(&td_entry, &td_element, &mut cascade, GetOptions::content()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rejected_ops() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let authority = test_cell_env();

    // Data
    let td_entry = EntryTestData::create();
    let td_element = ElementTestData::create();
    fill_db_rejected(&authority.env(), td_entry.store_entry_op.clone());
    fill_db_rejected(&authority.env(), td_element.any_store_element_op.clone());

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.env().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty().with_network(network, cache.env());
    assert_rejected(&td_entry, &td_element, &mut cascade, GetOptions::latest()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn check_can_handle_rejected_ops_in_cache() {
    observability::test_run().ok();

    // Environments
    let cache = test_cell_env();
    let authority = test_cell_env();

    // Data
    let td_entry = EntryTestData::create();
    let td_element = ElementTestData::create();
    fill_db_rejected(&cache.env(), td_entry.store_entry_op.clone());
    fill_db_rejected(&cache.env(), td_element.any_store_element_op.clone());

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.env().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty().with_network(network, cache.env());
    assert_rejected(&td_entry, &td_element, &mut cascade, GetOptions::latest()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn check_all_queries_still_work() {
    // TODO: Come up with a list of different states the authority could
    // have data in (updates, rejected, abandoned, nothing etc.)
    // then create an iterator that can put databases in these states and
    // run all the above queries on them.
    let cache = test_cache_env();
    let authority = test_cell_env();
    let vault = test_cell_env();

    let mut u = Unstructured::new(&NOISE);
    let entry = Entry::arbitrary(&mut u).unwrap();
    let mut new_headers = || {
        build_seq(
            &mut u,
            100,
            facts![
                holochain_zome_types::header::facts::new_entry_header(),
                holochain_zome_types::header::facts::header_for_entry(entry.clone()),
                holochain_types::header::facts::valid_agent(authority.keystore()),
            ],
        )
    };
    let headers = new_headers();
    let rejected_headers = new_headers();
    let f1 = || facts![dht_op::facts::op_of_type(DhtOpType::StoreEntry),];
    let valid =
        |e: &holochain_state::prelude::TestEnv| facts![dht_op::facts::op_is_valid(e.keystore())];
    let f2 = || facts![dht_op::facts::op_for_entry(entry.clone()),];
    let facts = |headers: Vec<Header>| {
        headers
            .into_iter()
            .map(|header| {
                facts![
                    f1(),
                    dht_op::facts::op_for_header(header),
                    valid(&authority),
                    f2()
                ]
            })
            .collect::<Vec<_>>()
    };
    let not_facts = || facts![f1(), not_(f2()), valid(&authority)];
    let noise_ops = build_seq(&mut u, 100, not_facts());
    check_seq(&noise_ops[..], not_facts()).unwrap();
    let ops = facts(headers.clone())
        .into_iter()
        .map(|mut f| f.build(&mut u))
        .collect::<Vec<DhtOp>>();
    for (op, fact) in ops.iter().zip(facts(headers.clone())) {
        fact.check(op).unwrap();
    }
    let rejected_ops = facts(rejected_headers.clone())
        .into_iter()
        .map(|mut f| f.build(&mut u))
        .collect::<Vec<DhtOp>>();
    for (op, fact) in rejected_ops.iter().zip(facts(rejected_headers.clone())) {
        fact.check(op).unwrap();
    }
    assert_eq!(ops.len(), 100);
    assert_eq!(noise_ops.len(), 100);

    for op in ops.into_iter().chain(noise_ops.clone()) {
        fill_db(&authority.env(), op.into_hashed());
    }
    for op in rejected_ops {
        fill_db_rejected(&authority.env(), op.into_hashed());
    }
    for op in noise_ops {
        fill_db(&vault.env(), op.into_hashed());
    }
    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.env().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty()
        .with_vault(vault.env().into())
        .with_network(network, cache.env());

    let _ = cascade
        .dht_get(entry.to_hash().into(), GetOptions::latest())
        .await
        .unwrap();
    holochain_state::prelude::dump_tmp(&cache.env());
    let cache_ops = fresh_reader_test(cache.env(), |t| {
        holochain_state::prelude::get_all_ops(&t, ValidationStatus::Valid)
    });
    let facts = facts![f1(), f2()];
    check_seq(&cache_ops[..], facts).unwrap();
    assert_eq!(cache_ops.len(), 100);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "todo"]
async fn check_all_queries_still_work_with_cache() {
    todo!()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "todo"]
async fn check_all_queries_still_work_with_scratch() {
    todo!()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_pending_data_isnt_returned() {
    observability::test_run().ok();

    // Environments
    let cache = test_cache_env();
    let authority = test_cell_env();
    let vault = test_cell_env();

    // Data
    let td_entry = EntryTestData::create();
    let td_element = ElementTestData::create();
    fill_db_pending(&authority.env(), td_entry.store_entry_op.clone());
    fill_db_pending(&authority.env(), td_element.any_store_element_op.clone());
    fill_db_pending(&vault.env(), td_entry.store_entry_op.clone());
    fill_db_pending(&vault.env(), td_element.any_store_element_op.clone());
    fill_db_pending(&cache.env(), td_entry.store_entry_op.clone());
    fill_db_pending(&cache.env(), td_element.any_store_element_op.clone());

    // Network
    let network = PassThroughNetwork::authority_for_nothing(vec![authority.env().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty().with_network(network, cache.env());

    assert_is_none(&td_entry, &td_element, &mut cascade, GetOptions::latest()).await;

    assert_can_retrieve(&td_entry, &mut cascade, GetOptions::latest()).await;

    let network = PassThroughNetwork::authority_for_all(vec![authority.env().clone().into()]);

    // Cascade
    let mut cascade = Cascade::empty().with_network(network, cache.env());

    assert_is_none(&td_entry, &td_element, &mut cascade, GetOptions::latest()).await;

    assert_can_retrieve(&td_entry, &mut cascade, GetOptions::latest()).await;
}
