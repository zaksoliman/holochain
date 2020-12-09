use std::{convert::TryInto, path::PathBuf};

use hdk3::prelude::CellId;
use holo_hash::{AgentPubKey, EntryHash};
use holochain_keystore::AgentPubKeyExt;
use holochain_serialized_bytes::prelude::*;
use holochain_types::{app::InstalledCell, dna::DnaFile};
use holochain_zome_types::{timestamp::Timestamp, ZomeCallResponse};
use kitsune_p2p::KitsuneP2pConfig;
use matches::assert_matches;

use super::local_network_tests::*;
use crate::{
    conductor::api::error::{ConductorApiError, ConductorApiResult},
    test_utils::{
        install_app, new_zome_call, setup_app_with_network, wait_for_integration_with_others, wait_for_integration,
    },
};
use test_case::test_case;
use tracing::*;

#[derive(Serialize, Deserialize, SerializedBytes, Clone, Debug)]
pub struct ChannelInfo {
    pub name: String,
    pub created_by: AgentPubKey,
    pub created_at: Timestamp,
}

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
pub struct ChannelListInput {
    category: String,
}

#[derive(Serialize, Deserialize, SerializedBytes, derive_more::From, Debug)]
pub struct ChannelList {
    channels: Vec<ChannelData>,
}

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
pub struct ChannelInput {
    name: String,
    channel: Channel,
}

#[derive(Serialize, Deserialize, SerializedBytes, derive_more::Constructor, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChannelData {
    pub channel: Channel,
    pub info: ChannelInfo,
    pub latest_chunk: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
pub struct Channel {
    category: String,
    uuid: String,
}

#[derive(Serialize, Deserialize, SerializedBytes, Clone, Debug)]
pub struct Message {
    pub uuid: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
pub enum LastSeen {
    First,
    Message(EntryHash),
}

#[derive(Serialize, Deserialize, SerializedBytes, Debug)]
pub struct MessageInput {
    pub last_seen: LastSeen,
    pub channel: Channel,
    pub message: Message,
    pub chunk: u32,
}
#[derive(Serialize, Deserialize, SerializedBytes)]
pub struct ListMessagesInput {
    channel: Channel,
    chunk: Chunk,
    active_chatter: bool,
}
#[derive(Serialize, Deserialize, SerializedBytes)]
pub struct Chunk {
    start: u32,
    end: u32,
}

#[derive(Serialize, Deserialize, SerializedBytes, derive_more::From)]
pub struct ListMessages {
    messages: Vec<MessageData>,
}

#[derive(Serialize, Deserialize, Clone, Debug, SerializedBytes)]
#[serde(rename_all = "camelCase")]
pub struct MessageData {
    message: Message,
    entry_hash: EntryHash,
    created_by: AgentPubKey,
    created_at: Timestamp,
}

#[test_case(2, 1, 1)]
#[test_case(5, 1, 1)]
#[test_case(1, 5, 5)]
#[test_case(5, 5, 5)]
#[test_case(1, 10, 1)]
#[test_case(1, 1, 1)]
#[test_case(10, 10, 10)]
#[test_case(8, 8, 8)]
#[test_case(10, 10, 1)]
#[ignore = "Don't want network tests running on ci"]
fn conductors_remote_boot_chat(
    num_committers: usize,
    num_conductors: usize,
    new_conductors: usize,
) {
    let mut network = KitsuneP2pConfig::default();
    let transport = kitsune_p2p::TransportConfig::Quic {
        bind_to: None,
        override_port: None,
        override_host: None,
    };
    network.bootstrap_service = Some(url2::url2!("https://bootstrap.holo.host/"));
    let proxy_config = if let Some(proxy_addr) = std::env::var_os("KIT_PROXY") {
        holochain_p2p::kitsune_p2p::ProxyConfig::RemoteProxyClient {
            proxy_url: url2::url2!("{}", proxy_addr.into_string().unwrap()),
        }
    } else {
        holochain_p2p::kitsune_p2p::ProxyConfig::RemoteProxyClient{
            proxy_url: url2::url2!("kitsune-proxy://CIW6PxKxsPPlcuvUCbMcKwUpaMSmB7kLD8xyyj4mqcw/kitsune-quic/h/proxy.holochain.org/p/5778/--"),
        }
    };
    network.transport_pool = vec![kitsune_p2p::TransportConfig::Proxy {
        sub_transport: transport.into(),
        proxy_config,
    }];
    let f = conductors_chat_inner(
        num_committers,
        num_conductors,
        new_conductors,
        network,
        false,
    );
    crate::conductor::tokio_runtime().block_on(f);
}

#[test_case(2, 1, 1)]
#[test_case(5, 1, 1)]
#[test_case(1, 5, 5)]
#[test_case(5, 5, 5)]
#[test_case(1, 10, 1)]
#[test_case(1, 1, 1)]
#[test_case(10, 10, 10)]
#[test_case(8, 8, 8)]
#[test_case(10, 10, 1)]
#[ignore = "Don't want network tests running on ci"]
fn conductors_remote_chat(num_committers: usize, num_conductors: usize, new_conductors: usize) {
    let mut network = KitsuneP2pConfig::default();
    let transport = kitsune_p2p::TransportConfig::Quic {
        bind_to: None,
        override_port: None,
        override_host: None,
    };
    let proxy_config = if let Some(proxy_addr) = std::env::var_os("KIT_PROXY") {
        holochain_p2p::kitsune_p2p::ProxyConfig::RemoteProxyClient {
            // Real proxy
            proxy_url: url2::url2!("{}", proxy_addr.into_string().unwrap()),
        }
    } else {
        holochain_p2p::kitsune_p2p::ProxyConfig::RemoteProxyClient{
            // Real proxy
            proxy_url: url2::url2!("kitsune-proxy://CIW6PxKxsPPlcuvUCbMcKwUpaMSmB7kLD8xyyj4mqcw/kitsune-quic/h/proxy.holochain.org/p/5778/--"),
            // Local proxy
            // proxy_url: url2::url2!("kitsune-proxy://h5_sQGIdBB7OnWVc1iuYZ-QUzb0DowdCA73PA0oOcv4/kitsune-quic/h/192.168.1.6/p/58451/--"),
            // Other machine proxy
            // proxy_url: url2::url2!("kitsune-proxy://h5_sQGIdBB7OnWVc1iuYZ-QUzb0DowdCA73PA0oOcv4/kitsune-quic/h/192.168.1.68/p/58451/--"),
        }
    };

    network.transport_pool = vec![kitsune_p2p::TransportConfig::Proxy {
        sub_transport: transport.into(),
        proxy_config,
    }];
    let f = conductors_chat_inner(
        num_committers,
        num_conductors,
        new_conductors,
        network,
        true,
    );
    crate::conductor::tokio_runtime().block_on(f);
}

async fn conductors_chat_inner(
    num_committers: usize,
    num_conductors: usize,
    new_conductors: usize,
    network: KitsuneP2pConfig,
    share_peers: bool,
) {
    observability::test_run().ok();
    let uuid = nanoid::nanoid!().to_string();

    let path = PathBuf::from("/home/tomgowan/holochain/elemental-chat/elemental-chat.dna.gz");

    let handles = setup_extern(
        Some(network.clone()),
        path.clone(),
        num_committers,
        uuid.clone(),
    )
    .await;

    let headers = create_channel_and_message(&handles[..]).await;

    let second_handles = setup_extern(
        Some(network.clone()),
        path.clone(),
        num_conductors,
        uuid.clone(),
    )
    .await;

    let mut envs = Vec::with_capacity(handles.len() + second_handles.len());
    for h in handles.iter().chain(second_handles.iter()) {
        envs.push(h.get_p2p_env().await);
    }

    if share_peers {
        exchange_peer_info(envs.clone());
    }

    // for _ in 0..600 {
    //     check_peers(envs.clone());
    //     tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
    // }

    let all_handles = handles
        .iter()
        .chain(second_handles.iter())
        .collect::<Vec<_>>();

    // 3 ops per create plus 7 for genesis + 2 for init + 2 for cap
    let mut expected_count = num_committers * (23 + 7 + 2 + 2) + num_conductors * (7 + 2 + 2);
    for (i, handle) in second_handles.iter().enumerate() {
        check_messages(handle, &all_handles, &headers, expected_count, line!(), i).await;
        // Add 4 ops for each init
        // expected_count += 4;
    }

    shutdown(handles).await;

    let third_handles =
        setup_extern(Some(network.clone()), path.clone(), new_conductors, uuid).await;

    let mut envs = Vec::with_capacity(third_handles.len() + second_handles.len());
    for h in third_handles.iter().chain(second_handles.iter()) {
        envs.push(h.get_p2p_env().await);
    }

    if share_peers {
        exchange_peer_info(envs.clone());
    }

    let all_handles = third_handles
        .iter()
        .chain(second_handles.iter())
        .collect::<Vec<_>>();

    expected_count += new_conductors * (7 + 2 + 2);
    for (i, handle) in third_handles.iter().enumerate() {
        check_messages(handle, &all_handles, &headers, expected_count, line!(), i).await;
        // Add 4 ops for each init
        expected_count += 4;
    }

    shutdown(second_handles).await;

    let all_handles = third_handles.iter().collect::<Vec<_>>();

    for (i, handle) in third_handles.iter().enumerate() {
        check_messages(handle, &all_handles, &headers, expected_count, line!(), i).await;
    }

    shutdown(third_handles).await;
}

async fn create_channel_and_message(handles: &[TestHandle]) -> Vec<MessageData> {
    let mut futures = Vec::with_capacity(handles.len());
    for (i, h) in handles.iter().cloned().enumerate() {
        let f = async move {
            let channel = Channel {
                category: "General".to_string(),
                uuid: nanoid::nanoid!(),
            };
            let invocation = new_zome_call(
                &h.cell_id,
                "create_channel",
                ChannelInput {
                    name: format!("Test Channel {}", i),
                    channel: channel.clone(),
                },
                "chat",
            )
            .unwrap();
            h.call_zome(invocation).await.unwrap().unwrap();
            let invocation = new_zome_call(
                &h.cell_id,
                "create_message",
                MessageInput {
                    channel: channel.clone(),
                    last_seen: LastSeen::First,
                    chunk: 0,
                    message: Message {
                        uuid: nanoid::nanoid!(),
                        content: "Hello from alice :)".to_string(),
                    },
                },
                "chat",
            )
            .unwrap();
            h.call_zome(invocation).await.unwrap().unwrap()
        };
        let f = tokio::task::spawn(f);
        futures.push(f);
    }
    let mut headers = Vec::with_capacity(handles.len());
    for f in futures {
        let result = f.await.unwrap();
        let result: MessageData = unwrap_to::unwrap_to!(result => ZomeCallResponse::Ok)
            .clone()
            .into_inner()
            .try_into()
            .unwrap();
        headers.push(result);
    }
    headers
}

async fn setup_extern(
    network: Option<KitsuneP2pConfig>,
    path: PathBuf,
    num_conductors: usize,
    uuid: String,
) -> Vec<TestHandle> {
    let dna_file = read_parse_dna(path).await.unwrap();
    let dna_file = dna_file.with_uuid(uuid).await.unwrap();

    let mut handles = Vec::with_capacity(num_conductors);
    for _ in 0..num_conductors {
        let dnas = vec![dna_file.clone()];
        let (__tmpdir, _, handle) =
            setup_app_with_network(vec![], vec![], network.clone().unwrap_or_default()).await;

        let agent_key = AgentPubKey::new_from_pure_entropy(handle.keystore())
            .await
            .unwrap();
        let cell_id = CellId::new(dna_file.dna_hash().to_owned(), agent_key.clone());
        let app = InstalledCell::new(cell_id.clone(), "cell_handle".into());
        install_app("test_app", vec![(app, None)], dnas.clone(), handle.clone()).await;
        handles.push(TestHandle {
            __tmpdir,
            cell_id,
            handle,
        });
    }
    handles
}

async fn read_parse_dna(dna_path: PathBuf) -> ConductorApiResult<DnaFile> {
    let dna_content = tokio::fs::read(dna_path)
        .await
        .map_err(|e| ConductorApiError::DnaReadError(format!("{:?}", e)))?;
    let dna = DnaFile::from_file_content(&dna_content).await?;
    Ok(dna)
}

pub async fn check_messages(
    handle: &TestHandle,
    all_handles: &[&TestHandle],
    messages: &[MessageData],
    expected_count: usize,
    line: u32,
    i: usize,
) {
    const NUM_ATTEMPTS: usize = 600;
    const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(100);

    let mut others = Vec::with_capacity(all_handles.len());
    for other in all_handles {
        let other = other.get_cell_env(&other.cell_id).await.unwrap();
        others.push(other);
    }
    let others_ref = others.iter().collect::<Vec<_>>();

    // wait_for_integration(
    //     &handle.get_cell_env(&handle.cell_id).await.unwrap(),
    //     expected_count,
    //     NUM_ATTEMPTS,
    //     DELAY_PER_ATTEMPT.clone(),
    // ).await;
    wait_for_integration_with_others(
        &handle.get_cell_env(&handle.cell_id).await.unwrap(),
        &others_ref,
        expected_count,
        NUM_ATTEMPTS,
        DELAY_PER_ATTEMPT.clone(),
    )
    .await;
    for message in messages {
        let invocation = new_zome_call(
            &handle.cell_id,
            "list_channels",
            ChannelListInput {
                category: "General".to_string(),
            },
            "chat",
        )
        .unwrap();
        let result = handle.call_zome(invocation).await.unwrap().unwrap();
        let result: ChannelList = unwrap_to::unwrap_to!(result => ZomeCallResponse::Ok)
            .clone()
            .into_inner()
            .try_into()
            .unwrap();

        let channel = result.channels.get(0);
        assert_matches!(channel, Some(ChannelData { .. }));
        let channel = channel.unwrap().channel.clone();
        let s = debug_span!("check_message", ?line, ?i, ?message);
        let _g = s.enter();
        // assert_matches!(result.into_inner(), Some(_));
        tracing::debug!(?channel);
        let invocation = new_zome_call(
            &handle.cell_id,
            "list_messages",
            ListMessagesInput {
                channel,
                chunk: Chunk { start: 0, end: 0 },
                active_chatter: false,
            },
            "chat",
        )
        .unwrap();
        let result = handle.call_zome(invocation).await.unwrap().unwrap();
        let messages: ListMessages = unwrap_to::unwrap_to!(result => ZomeCallResponse::Ok)
            .clone()
            .into_inner()
            .try_into()
            .unwrap();
        tracing::debug!(?message);
        assert_eq!(messages.messages.len(), 1);
        assert_eq!(messages.messages[0].entry_hash, message.entry_hash);
    }
}
