//! Fixture data to be used in gossip tests

use std::collections::HashMap;

use super::lazy_buckets::LazyBuckets;
use holo_hash::*;
use holochain_types::prelude::*;
use kitsune_p2p::test_util::scenario_def::LocBucket;
use parking_lot::{Mutex, MutexGuard};

const FIXTURE_PATH: &'static str = "fixtures/gossip-fixtures.msgpack";

/// Fixture data to be used in gossip tests.
/// The ops and agents are generated such that their DHT locations are
/// evenly distributed around the u32 location space.
pub struct GossipFixtures {
    /// The pre-generated Agent keys and Ops.
    /// One set of ops per agent.
    buckets: LazyBuckets<(AgentPubKey, LazyBuckets<DhtOpHashed>)>,
}

impl GossipFixtures {
    pub fn new(keystore: KeystoreSender, u: &mut arbitrary::Unstructured<'static>) -> Self {
        use arbitrary::Arbitrary;
        use contrafact::*;

        let agents = LazyBuckets::new(
            || {
                let agent = AgentPubKey::arbitrary(u).unwrap();
                let op_fact =
                    holochain_types::dht_op::facts::valid_dht_op_by_author(keystore, agent);
                let ops = LazyBuckets::new(
                    || {
                        let op = op_fact.build(u);
                        HoloHashed::from_content_sync(op)
                    },
                    |op| op.dht_basis().get_loc(),
                );
                (agent, ops)
            },
            |agent: &AgentPubKey| agent.get_loc(),
        );
        Self(agents)
    }

    pub fn agent(&mut self, i: LocBucket) -> &AgentPubKey {
        &self.buckets.get(i).0
    }

    pub fn ops_by_loc(&mut self, i: LocBucket) -> &LazyBuckets<DhtOpHashed> {
        &self.buckets.get(i).1
    }

    pub fn ops_by_agent(&mut self, agent: &AgentPubKey) -> &LazyBuckets<DhtOpHashed> {
        &self
            .0
            .find(|b| b.0 == agent)
            .expect("agent key not found")
            .1
    }

    pub fn lookup<'a>(
        &self,
        hashes: impl Iterator<Item = &'a DhtOpHash>,
    ) -> impl Iterator<Item = LocBucket> {
        // NB: this is expensive! Find a better way.
        let all: HashMap<DhtOpHash, LocBucket> = self
            .buckets
            .items
            .iter()
            // filter out the Nones
            .flat_map(|x| x)
            // Build reverse lookup
            .map(|(agent, ops)| {
                ops.items
                    .iter()
                    .flat_map(|x| x)
                    .enumerate()
                    .map(|(loc, hash)| (hash, loc))
            })
            .flatten()
            .collect();
        hashes.map(|h| {
            all.get(h)
                .expect("Couldn't lookup() hash not present in GossipFixtures")
                .clone()
        })
    }
}

pub fn initialize_gossip_fixtures(keystore: KeystoreSender) {
    let g = GOSSIP_FIXTURES.lock();
    let _ = std::mem::replace(
        g,
        GossipFixtures::new(keystore, &mut arbitrary::Unstructured::new(&NOISE)),
    );
}

pub fn gossip_fixtures() -> MutexGuard<'static, GossipFixtures> {
    GOSSIP_FIXTURES.lock()
}

/// Lazily-instantiated fixtures for gossip.
/// This can take some time to generate the first time, since it is a brute-force
/// search for hashes that satisfy the necessary conditions. The result is
/// cached on disk for subsequent runs.
static GOSSIP_FIXTURES: once_cell::sync::Lazy<Mutex<GossipFixtures>> =
    once_cell::sync::Lazy::new(|| {
        let uninitialized = GossipFixtures {
            buckets: LazyBuckets::new(
                || {
                    unreachable!(
                    "Must call `initialize_gossip_fixtures` before using singleton GossipFixtures"
                )
                },
                |_| {
                    unreachable!(
                    "Must call `initialize_gossip_fixtures` before using singleton GossipFixtures"
                )
                },
            ),
        };
        Mutex::new(uninitialized)
    });

// /// Map from DhtOpHash to the CoarseLoc for the **basis hash** (not the op hash!)
// pub static GOSSIP_FIXTURE_OP_LOOKUP: once_cell::sync::Lazy<HashMap<DhtOpHash, LocBucket>> =
//     once_cell::sync::Lazy::new(|| {
//         GOSSIP_FIXTURES
//             .ops
//             .items
//             .iter()
//             .enumerate()
//             .map(|(i, op)| (op.as_hash().clone(), i as i8))
//             .collect()
//     });
