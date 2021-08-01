use crate::capability::CapSecret;
use crate::prelude::*;
use crate::zome::FunctionName;
use crate::zome::ZomeName;
use holo_hash::AgentPubKey;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CallRemote {
    pub target_agent: AgentPubKey,
    pub zome_name: ZomeName,
    pub fn_name: FunctionName,
    pub cap: Option<CapSecret>,
    pub payload: ExternIO,
}

impl CallRemote {
    pub fn new(
        target_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        payload: ExternIO,
    ) -> Self {
        Self {
            target_agent,
            zome_name,
            fn_name,
            cap,
            payload,
        }
    }

    pub fn target_agent_as_ref(&self) -> &AgentPubKey {
        &self.target_agent
    }

    pub fn zome_name_as_ref(&self) -> &ZomeName {
        &self.zome_name
    }

    pub fn fn_name_as_ref(&self) -> &FunctionName {
        &self.fn_name
    }

    pub fn cap_as_ref(&self) -> &Option<CapSecret> {
        &self.cap
    }

    pub fn payload_as_ref(&self) -> &ExternIO {
        &self.payload
    }
}
