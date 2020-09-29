use super::CapSecret;
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;

/// System entry to hold a capability token claim for use as a caller.
/// Stored by a claimant so they can remember what's necessary to exercise
/// this capability by sending the secret to the grantor.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, SerializedBytes)]
pub struct CapClaim {
    /// A string by which to later query for saved claims.
    /// This does not need to be unique within a source chain.
    tag: String,
    /// AgentPubKey of agent who authored the corresponding CapGrant.
    grantor: AgentPubKey,
    /// The secret needed to exercise this capability.
    /// This is the only bit sent over the wire to attempt a remote call.
    /// Note that the grantor may have revoked the corresponding grant since we received the claim
    /// so claims are only ever a 'best effort' basis.
    secret: CapSecret,
}

impl CapClaim {
    /// Constructor.
    pub fn new(tag: String, grantor: AgentPubKey, secret: CapSecret) -> Self {
        CapClaim {
            tag,
            grantor,
            secret,
        }
    }

    /// Access the secret.
    pub fn secret(&self) -> &CapSecret {
        &self.secret
    }

    /// Access the tag
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Access the grantor
    pub fn grantor(&self) -> &AgentPubKey {
        &self.grantor
    }
}

/// The different ways to provide a capability claim for a zome call
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ZomeCallCapClaim {
    /// No capability claimed: expecting Unrestricted access to this function
    None,
    /// Claiming access to a LocalOnly grant: this request originated locally, across an interface
    Local(CapSecret),
    /// Claiming access to a Transferable grant: this request originated from an agent on the network
    Remote(CapSecret),
}

impl ZomeCallCapClaim {
    /// Constructor for Local or None variants
    pub fn local(secret: Option<CapSecret>) -> Self {
        secret.map(Self::Local).unwrap_or(Self::None)
    }

    /// Constructor for Remote or None variants
    pub fn remote(secret: Option<CapSecret>) -> Self {
        secret.map(Self::Remote).unwrap_or(Self::None)
    }

    /// Access the secret, if applicable
    pub fn secret(&self) -> Option<&CapSecret> {
        match self {
            Self::Local(secret) | Self::Remote(secret) => Some(secret),
            Self::None => None,
        }
    }
}
