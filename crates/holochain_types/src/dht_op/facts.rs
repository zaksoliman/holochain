//! Facts about DhtOps

use super::*;
use ::contrafact::*;
use unwrap_to::unwrap_to;

/// Fact: The DhtOp is internally consistent in all of its references:
/// - The Signature matches the Header
/// - If the header references an Entry, the Entry will exist
///     and be of the appropriate hash
/// - If the header does not reference an Entry, the entry will be None
/// - The DhtOp variant matches the Header variant (It is actually impossible to
///     violate this constraint due to the types, so no check is needed here.)
pub fn op_is_valid(keystore: KeystoreSender) -> Facts<'static, DhtOp> {
    facts![
        brute("Header type matches Entry existence", |op: &DhtOp| {
            let has_header = op.header().entry_data().is_some();
            let has_entry = op.entry().is_some();
            has_header == has_entry
        }),
        mapped(
            "If there is entry data, the header must point to it",
            |op: &DhtOp| {
                if let Some(entry) = op.entry() {
                    // NOTE: this could be a `lens` if the previous check were short-circuiting,
                    // but it is possible that this check will run even if the previous check fails,
                    // so use a prism instead.
                    facts![prism(
                        "header's entry hash",
                        |op: &mut DhtOp| op.header_entry_data_mut().map(|(hash, _)| hash),
                        eq("hash of matching entry", EntryHash::with_data_sync(entry)),
                    )]
                } else {
                    facts![always()]
                }
            }
        ),
        lens(
            "author",
            |op: &mut DhtOp| op.author_mut(),
            agent_in_keystore(keystore.clone())
        ),
        mapped("The Signature matches the Header", move |op: &DhtOp| {
            use holochain_keystore::AgentPubKeyExt;
            let header = op.header();
            let agent = header.author();
            let actual = match tokio_helper::block_forever_on(agent.sign(&keystore, &header)) {
                Ok(a) => a,
                Err(_) => return facts![never("Signing failed")],
            };
            facts![lens("signature", DhtOp::signature_mut, eq_(actual))]
        })
    ]
}

/// Fact: this DhtOp is about a given Header.
/// If the Header is of the wrong type for the op, panic.
pub fn op_of_type(op_type: DhtOpType) -> Facts<'static, DhtOp> {
    facts![brute("DhtOp is of given type", move |op: &DhtOp| op
        .get_type()
        == op_type)]
}

/// Fact: this DhtOp is about a given Header.
/// If the Header is of the wrong type for the op, panic.
pub fn op_for_header(header: Header) -> Facts<'static, DhtOp> {
    facts![OpForHeader(header)]
}

/// Fact: this DhtOp has a given Entry.
/// If the op is of the wrong type for the entry, panic.
pub fn op_for_entry(entry: Entry) -> Facts<'static, DhtOp> {
    facts![OpForEntry(entry)]
}

/// Fact: this agent is registered with the keystore.
// TODO: this probably belongs in holochain_keystore
pub fn agent_in_keystore(keystore: KeystoreSender) -> Facts<'static, AgentPubKey> {
    use holochain_keystore::KeystoreSenderExt;
    facts![mapped(
        "Agent is in keystore",
        move |agent: &AgentPubKey| {
            if tokio_helper::block_forever_on(keystore.sign(Sign::new_raw(agent.clone(), vec![])))
                .is_ok()
            {
                // If we can sign, the agent is already in the keystore:
                // no constraint needed
                facts![always()]
            } else {
                // If not, we have to create a new one and add a constraint
                facts![eq(
                    "new agent",
                    tokio_helper::block_forever_on(AgentPubKey::new_from_pure_entropy(&keystore))
                        .unwrap(),
                )]
            }
        }
    )]
}

/// Create a valid dht op with a header and maybe an entry.
/// Op type must make sense with the header type and entry.
/// Header must make sense with the entry.
pub fn valid_op_with_header_and_entry(
    keystore: KeystoreSender,
    op_type: DhtOpType,
    header: Header,
    entry: Option<Entry>,
) -> Facts<'static, DhtOp> {
    let facts = facts![
        op_of_type(op_type),
        op_for_header(header),
        op_is_valid(keystore),
    ];
    match entry {
        Some(entry) => facts![facts, op_for_entry(entry),],
        None => facts,
    }
}

/// Produces a valid chain of `StoreElement` ops with the same author.
pub fn valid_chain_of_ops(author: AgentPubKey, keystore: KeystoreSender) -> Facts<'static, DhtOp> {
    facts![
        op_of_type(DhtOpType::StoreElement),
        prism(
            "valid_op_chain",
            |op: &mut DhtOp| op.header_mut(),
            facts![
                header::facts::valid_chain(),
                crate::header::facts::is_same_agent(author)
            ]
        ),
        op_is_valid(keystore),
    ]
}

struct OpForHeader(Header);

struct OpForEntry(Entry);

impl Fact<DhtOp> for OpForHeader {
    fn check(&self, op: &DhtOp) -> contrafact::Check {
        Check::check(
            op.header() == self.0,
            format!("Header does not match: {:?} != {:?}", op.header(), self.0),
        )
    }

    fn mutate(&self, op: &mut DhtOp, _: &mut arbitrary::Unstructured<'static>) {
        match op {
            DhtOp::StoreElement(_, header, _) => *header = self.0.clone(),
            DhtOp::StoreEntry(_, header, _) => *header = self.0.clone().try_into().unwrap(),
            DhtOp::RegisterAgentActivity(_, header) => *header = self.0.clone(),
            DhtOp::RegisterUpdatedContent(_, header, _) => {
                *header = unwrap_to!(self.0 => Header::Update).clone()
            }
            DhtOp::RegisterUpdatedElement(_, header, _) => {
                *header = unwrap_to!(self.0 => Header::Update).clone()
            }
            DhtOp::RegisterDeletedBy(_, header) => {
                *header = unwrap_to!(self.0 => Header::Delete).clone()
            }
            DhtOp::RegisterDeletedEntryHeader(_, header) => {
                *header = unwrap_to!(self.0 => Header::Delete).clone()
            }
            DhtOp::RegisterAddLink(_, header) => {
                *header = unwrap_to!(self.0 => Header::CreateLink).clone()
            }
            DhtOp::RegisterRemoveLink(_, header) => {
                *header = unwrap_to!(self.0 => Header::DeleteLink).clone()
            }
        }
    }

    fn advance(&mut self, _: &DhtOp) {}
}

impl Fact<DhtOp> for OpForEntry {
    fn check(&self, op: &DhtOp) -> contrafact::Check {
        match op {
            DhtOp::StoreElement(_, _, Some(entry)) | DhtOp::StoreEntry(_, _, entry) => {
                if **entry == self.0 {
                    Check::pass()
                } else {
                    Check::fail(format!("Entry does not match: {:?} != {:?}", entry, self.0))
                }
            }
            _ => Check::fail(format!("Op doesn't contain an entry: {:?}", op)),
        }
    }

    fn mutate(&self, op: &mut DhtOp, _: &mut arbitrary::Unstructured<'static>) {
        match op {
            DhtOp::StoreElement(_, _, Some(entry)) => **entry = self.0.clone(),
            DhtOp::StoreEntry(_, _, entry) => **entry = self.0.clone(),
            _ => (),
        }
    }

    fn advance(&mut self, _: &DhtOp) {}
}

#[cfg(test)]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use holochain_keystore::test_keystore::spawn_test_keystore;

    use super::*;
    use holochain_zome_types::header::facts as header_facts;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_op_is_valid() {
        let mut uu = Unstructured::new(&NOISE);
        let u = &mut uu;
        let keystore = spawn_test_keystore().await.unwrap();
        let agent = AgentPubKey::new_from_pure_entropy(&keystore).await.unwrap();

        let e = Entry::arbitrary(u).unwrap();

        let mut hn = not_(header_facts::is_new_entry_header()).build(u);
        *hn.author_mut() = agent.clone();

        let mut he = header_facts::is_new_entry_header().build(u);
        *he.entry_data_mut().unwrap().0 = EntryHash::with_data_sync(&e);
        let mut he = Header::from(he);
        *he.author_mut() = agent.clone();

        let se = agent.sign(&keystore, &he).await.unwrap();
        let sn = agent.sign(&keystore, &hn).await.unwrap();

        let op1 = DhtOp::StoreElement(se.clone(), he.clone(), Some(Box::new(e.clone())));
        let op2 = DhtOp::StoreElement(se.clone(), he.clone(), None);
        let op3 = DhtOp::StoreElement(sn.clone(), hn.clone(), Some(Box::new(e.clone())));
        let op4 = DhtOp::StoreElement(sn.clone(), hn.clone(), None);

        let fact = op_is_valid(keystore);

        fact.check(&op1).unwrap();
        assert!(fact.check(&op2).is_err());
        assert!(fact.check(&op3).is_err());
        fact.check(&op4).unwrap();
    }
}

impl DhtOp {
    /// Mutable access to the Signature
    pub fn signature_mut(&mut self) -> &mut Signature {
        match self {
            DhtOp::StoreElement(s, _, _) => s,
            DhtOp::StoreEntry(s, _, _) => s,
            DhtOp::RegisterAgentActivity(s, _) => s,
            DhtOp::RegisterUpdatedContent(s, _, _) => s,
            DhtOp::RegisterUpdatedElement(s, _, _) => s,
            DhtOp::RegisterDeletedBy(s, _) => s,
            DhtOp::RegisterDeletedEntryHeader(s, _) => s,
            DhtOp::RegisterAddLink(s, _) => s,
            DhtOp::RegisterRemoveLink(s, _) => s,
        }
    }

    /// Mutable access to the seq of the Header, if applicable
    pub fn header_seq_mut(&mut self) -> Option<&mut u32> {
        match self {
            DhtOp::StoreElement(_, ref mut h, _) => h.header_seq_mut(),
            DhtOp::StoreEntry(_, ref mut h, _) => Some(h.header_seq_mut()),
            DhtOp::RegisterAgentActivity(_, ref mut h) => h.header_seq_mut(),
            DhtOp::RegisterUpdatedContent(_, ref mut h, _) => Some(&mut h.header_seq),
            DhtOp::RegisterUpdatedElement(_, ref mut h, _) => Some(&mut h.header_seq),
            DhtOp::RegisterDeletedBy(_, ref mut h) => Some(&mut h.header_seq),
            DhtOp::RegisterDeletedEntryHeader(_, ref mut h) => Some(&mut h.header_seq),
            DhtOp::RegisterAddLink(_, ref mut h) => Some(&mut h.header_seq),
            DhtOp::RegisterRemoveLink(_, ref mut h) => Some(&mut h.header_seq),
        }
    }

    /// Mutable access to the Header, if applicable
    pub fn header_mut(&mut self) -> Option<&mut Header> {
        match self {
            DhtOp::StoreElement(_, ref mut h, _) => Some(h),
            DhtOp::RegisterAgentActivity(_, ref mut h) => Some(h),
            _ => None,
        }
    }

    /// Mutable access to the author of the Header
    pub fn author_mut(&mut self) -> &mut AgentPubKey {
        match self {
            DhtOp::StoreElement(_, ref mut h, _) => h.author_mut(),
            DhtOp::StoreEntry(_, ref mut h, _) => (h.author_mut()),
            DhtOp::RegisterAgentActivity(_, ref mut h) => h.author_mut(),
            DhtOp::RegisterUpdatedContent(_, ref mut h, _) => (&mut h.author),
            DhtOp::RegisterUpdatedElement(_, ref mut h, _) => (&mut h.author),
            DhtOp::RegisterDeletedBy(_, ref mut h) => (&mut h.author),
            DhtOp::RegisterDeletedEntryHeader(_, ref mut h) => (&mut h.author),
            DhtOp::RegisterAddLink(_, ref mut h) => (&mut h.author),
            DhtOp::RegisterRemoveLink(_, ref mut h) => (&mut h.author),
        }
    }

    /// Mutable access to the entry data of the Header, if applicable
    pub fn header_entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)> {
        match self {
            DhtOp::StoreElement(_, ref mut h, _) => h.entry_data_mut(),
            DhtOp::StoreEntry(_, ref mut h, _) => Some(h.entry_data_mut()),
            DhtOp::RegisterAgentActivity(_, ref mut h) => h.entry_data_mut(),
            DhtOp::RegisterUpdatedContent(_, ref mut h, _) => {
                Some((&mut h.entry_hash, &mut h.entry_type))
            }
            DhtOp::RegisterUpdatedElement(_, ref mut h, _) => {
                Some((&mut h.entry_hash, &mut h.entry_type))
            }
            _ => None,
        }
    }
}
