use crate::capability::CapSecret;
use crate::zome::FunctionName;
use crate::zome::ZomeName;
use crate::ExternIO;
use holo_hash::AgentPubKey;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CallRoute {
    /// Same DNA, same agent.
    Local,
    /// Same DNA, different agent.
    NetworkAgent(AgentPubKey),
    /// Same agent, different DNA. Specified Hash.
    DnaHash(DnaHash),
    /// Same agent, different DNA. Configurable role.
    DnaRole(DnaRole),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, Constructor)]
pub struct Call {
    pub route: CallRoute,
    pub zome_role: ZomeRole,
    pub fn_role: FunctionRole,
    pub cap_secret: Option<CapSecret>,
    pub payload: ExternIO,
}