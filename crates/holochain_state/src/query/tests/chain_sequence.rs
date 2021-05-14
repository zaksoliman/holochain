use std::sync::Arc;

use super::*;
use crate::prelude::*;
use crate::query::chain_head::ChainHeadQuery;
use crate::test_utils::test_cell_env;
use contrafact::arbitrary::Unstructured;
use holo_hash::AgentPubKey;
use holo_hash::HashableContentExtSync;
use holochain_types::dht_op;
use holochain_types::dht_op::facts::valid_chain_of_ops;
use observability;
use tokio::sync::Barrier;

#[tokio::test(flavor = "multi_thread")]
async fn chain_sequence_scratch_awareness() {
    observability::test_run().ok();
    let test_env = test_cell_env();
    let vault = test_env.env();
    let mut u = Unstructured::new(&NOISE);

    let author: AgentPubKey = dht_op::facts::agent_in_keystore(vault.keystore()).build(&mut u);
    let ops = build_seq(
        &mut u,
        100,
        valid_chain_of_ops(author.clone(), vault.keystore()),
    );

    let chain_head = ChainHeadQuery::new(Arc::new(author));
    vault
        .conn()
        .unwrap()
        .with_commit_test(|txn| {
            assert!(chain_head.run(Txn::from(&(*txn))).unwrap().is_none());
            for op in &ops {
                mutations_helpers::insert_valid_authored_op(txn, op.clone().into_hashed()).unwrap();
                let (last_header, last_header_seq) = chain_head
                    .run(Txn::from(&(*txn)))
                    .unwrap()
                    .expect("Chain should not be empty now");
                assert_eq!(last_header, op.header().to_hash());
                assert_eq!(last_header_seq, op.header().header_seq());
            }
        })
        .unwrap();
}

/// If we attempt to move the chain head, but it has already moved from
/// under us, error
#[tokio::test(flavor = "multi_thread")]
async fn chain_sequence_head_moved_triggers_error() {
    let test_env = test_cell_env();
    let vault = test_env.env();
    let mut u = Unstructured::new(&NOISE);

    let author: AgentPubKey = dht_op::facts::agent_in_keystore(vault.keystore()).build(&mut u);

    source_chain::genesis(
        vault.clone(),
        DnaHash::arbitrary(&mut u).unwrap(),
        author.clone(),
        None,
    )
    .await
    .unwrap();

    let ops = build_seq(
        &mut u,
        100,
        valid_chain_of_ops(author.clone(), vault.keystore()),
    );

    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::with_capacity(2);
    let h = tokio::spawn({
        let barrier = barrier.clone();
        let ops = ops.clone();
        let vault = vault.clone();
        let author = author.clone();
        async move {
            let sc = SourceChain::new(vault.clone().into(), author.clone()).unwrap();
            barrier.wait().await;
            let scratch = sc.scratch();
            scratch
                .apply(|scratch| {
                    for op in &ops {
                        mutations::insert_op_scratch(scratch, op.clone().into_hashed()).unwrap()
                    }
                })
                .unwrap();
            sc.flush().unwrap();
            barrier.wait().await;
        }
    });
    handles.push(h);
    let h = tokio::spawn({
        let barrier = barrier.clone();
        let ops = ops.clone();
        let vault = vault.clone();
        let author = author.clone();
        async move {
            let sc = SourceChain::new(vault.clone().into(), author.clone()).unwrap();
            barrier.wait().await;
            let scratch = sc.scratch();
            scratch
                .apply(|scratch| {
                    for op in &ops {
                        mutations::insert_op_scratch(scratch, op.clone().into_hashed()).unwrap()
                    }
                })
                .unwrap();
            barrier.wait().await;
            matches::assert_matches!(sc.flush(), Err(SourceChainError::HeadMoved(_, _)));
        }
    });
    handles.push(h);
    for h in handles {
        h.await.unwrap();
    }
}

/// If the chain head has moved from under us, but we are not moving the
/// chain head ourselves, proceed as usual
#[tokio::test(flavor = "multi_thread")]
async fn chain_sequence_head_moved_triggers_no_error_if_clean() {
    let test_env = test_cell_env();
    let vault = test_env.env();
    let mut u = Unstructured::new(&NOISE);

    let author: AgentPubKey = dht_op::facts::agent_in_keystore(vault.keystore()).build(&mut u);

    source_chain::genesis(
        vault.clone(),
        DnaHash::arbitrary(&mut u).unwrap(),
        author.clone(),
        None,
    )
    .await
    .unwrap();

    let ops = build_seq(
        &mut u,
        100,
        valid_chain_of_ops(author.clone(), vault.keystore()),
    );

    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::with_capacity(2);
    let h = tokio::spawn({
        let barrier = barrier.clone();
        let ops = ops.clone();
        let vault = vault.clone();
        let author = author.clone();
        async move {
            let sc = SourceChain::new(vault.clone().into(), author.clone()).unwrap();
            barrier.wait().await;
            let scratch = sc.scratch();
            scratch
                .apply(|scratch| {
                    for op in &ops {
                        mutations::insert_op_scratch(scratch, op.clone().into_hashed()).unwrap()
                    }
                })
                .unwrap();
            sc.flush().unwrap();
            barrier.wait().await;
        }
    });
    handles.push(h);
    let h = tokio::spawn({
        let barrier = barrier.clone();
        let vault = vault.clone();
        let author = author.clone();
        async move {
            let sc = SourceChain::new(vault.clone().into(), author.clone()).unwrap();
            barrier.wait().await;
            barrier.wait().await;
            sc.flush()
                .expect("Source chain is clean so expect no error");
        }
    });
    handles.push(h);
    for h in handles {
        h.await.unwrap();
    }
}
