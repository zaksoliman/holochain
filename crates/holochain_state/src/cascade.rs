//! # Cascade
//! ## Retrieve vs Get
//! Get checks CRUD metadata before returning an the data
//! where as retrieve only checks that where the data was found
//! the appropriate validation has been run.

use super::{
    element_buf::ElementBuf,
    metadata::{ChainItemKey, LinkMetaKey, MetadataBuf, MetadataBufT},
};
use crate::core::workflow::integrate_dht_ops_workflow::integrate_single_metadata;
use either::Either;
use error::CascadeResult;
use fallible_iterator::FallibleIterator;
use holo_hash::{hash_type::AnyDht, AgentPubKey, AnyDhtHash, EntryHash, HasHash, HeaderHash};
use holochain_lmdb::{error::DatabaseResult, fresh_reader, prelude::*};
use holochain_p2p::{actor::GetActivityOptions, HolochainP2pCellT};
use holochain_p2p::{
    actor::{GetLinksOptions, GetMetaOptions, GetOptions},
    HolochainP2pCell,
};
use holochain_types::{
    activity::{AgentActivity, ChainItems},
    chain::AgentActivityExt,
    dht_op::{produce_op_lights_from_element_group, produce_op_lights_from_elements},
    element::{
        Element, ElementGroup, ElementStatus, GetElementResponse, RawGetEntryResponse,
        SignedHeaderHashed, SignedHeaderHashedExt,
    },
    entry::option_entry_hashed,
    link::{GetLinksResponse, WireLinkMetaKey},
    metadata::{EntryDhtStatus, MetadataSet, TimedHeaderHash},
    EntryHashed, HeaderHashed,
};
use holochain_zome_types::{
    element::SignedHeader,
    header::HeaderType,
    link::Link,
    metadata::{Details, ElementDetails, EntryDetails},
    query::ChainQueryFilter,
    query::ChainStatus,
    validate::{ValidationPackage, ValidationStatus},
};
use std::collections::HashSet;
use std::collections::{BTreeMap, BTreeSet};
use tracing::*;
use tracing_futures::Instrument;

/// A pair containing an element buf and metadata buf
/// with the same prefix.
/// The default IntegratedPrefix is for databases that don't
/// actually use prefixes (like the cache). In this case we just
/// choose the first one (IntegratedPrefix)
#[derive(derive_more::Constructor)]
pub struct DbPair<'a, M, P = IntegratedPrefix>
where
    P: PrefixType,
    M: MetadataBufT<P>,
{
    pub element: &'a ElementBuf<P>,
    pub meta: &'a M,
}

#[derive(derive_more::Constructor)]
pub struct DbPairMut<'a, M, P = IntegratedPrefix>
where
    P: PrefixType,
    M: MetadataBufT<P>,
{
    pub element: &'a mut ElementBuf<P>,
    pub meta: &'a mut M,
}

/// A version of the Cascade with no reference to the network, because
/// holochain_p2p is downstream of this crate
pub struct CascadeLocal<
    'a,
    MetaVault = MetadataBuf,
    MetaAuthored = MetadataBuf<AuthoredPrefix>,
    MetaCache = MetadataBuf,
    MetaPending = MetadataBuf<PendingPrefix>,
    MetaRejected = MetadataBuf<RejectedPrefix>,
> where
    MetaVault: MetadataBufT,
    MetaAuthored: MetadataBufT<AuthoredPrefix>,
    MetaPending: MetadataBufT<PendingPrefix>,
    MetaRejected: MetadataBufT<RejectedPrefix>,
    MetaCache: MetadataBufT,
{
    integrated_data: Option<DbPair<'a, MetaVault, IntegratedPrefix>>,
    authored_data: Option<DbPair<'a, MetaAuthored, AuthoredPrefix>>,
    pending_data: Option<DbPair<'a, MetaPending, PendingPrefix>>,
    rejected_data: Option<DbPair<'a, MetaRejected, RejectedPrefix>>,
    cache_data: Option<DbPairMut<'a, MetaCache>>,
    env: Option<EnvironmentRead>,
}
