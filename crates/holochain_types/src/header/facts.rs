use super::*;
use contrafact::*;

impl NewEntryHeader {
    pub fn header_seq_mut(&mut self) -> &mut u32 {
        match self {
            Self::Create(Create {
                ref mut header_seq, ..
            }) => header_seq,
            Self::Update(Update {
                ref mut header_seq, ..
            }) => header_seq,
        }
    }

    pub fn author_mut(&mut self) -> &mut AgentPubKey {
        match self {
            Self::Create(Create { ref mut author, .. }) => author,
            Self::Update(Update { ref mut author, .. }) => author,
        }
    }

    pub fn entry_data_mut(&mut self) -> (&mut EntryHash, &mut EntryType) {
        match self {
            Self::Create(Create {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => (entry_hash, entry_type),
            Self::Update(Update {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => (entry_hash, entry_type),
        }
    }

    pub fn entry_hash_mut(&mut self) -> &mut EntryHash {
        self.entry_data_mut().0
    }
}

pub fn update_with_entry_and_valid_author(
    entry: Entry,
    keystore: KeystoreSender,
) -> Facts<'static, Header> {
    facts![
        header::facts::is_of_type(HeaderType::Update),
        header::facts::header_for_entry(entry.clone()),
        valid_agent(keystore)
    ]
}

pub fn valid_agent(keystore: KeystoreSender) -> Facts<'static, Header> {
    facts![lens(
        "author",
        |header: &mut Header| header.author_mut(),
        crate::dht_op::facts::agent_in_keystore(keystore),
    )]
}

pub fn is_same_agent(agent: AgentPubKey) -> Facts<'static, Header> {
    facts![lens(
        "author",
        |header: &mut Header| header.author_mut(),
        eq_(agent)
    )]
}
