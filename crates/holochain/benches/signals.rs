use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;

use fixt::prelude::*;
use holo_hash::AgentPubKey;
use holochain::sweettest::*;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::AdminInterfaceConfig;
use holochain_conductor_api::InterfaceDriver;
use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
use holochain_types::prelude::InstallAppBundlePayload;
use holochain_types::prelude::InstalledApp;
use holochain_types::prelude::{AppBundleSource, DnaFile};
use holochain_types::signal::Signal;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::fixt::*;
use holochain_zome_types::CallRemote;
use holochain_zome_types::CapGrantEntry;
use holochain_zome_types::Entry;
use holochain_zome_types::EntryDefId;
use holochain_zome_types::EntryWithDefId;
use holochain_zome_types::ExternIO;
use holochain_zome_types::GrantedFunctions;
use holochain_zome_types::InitCallbackResult;
use holochain_zome_types::RemoteSignal;
use holochain_zome_types::ZomeCallResponse;
use kitsune_p2p::KitsuneP2pConfig;
use kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams;
use matches::assert_matches;
use reqwest::Client;
use tokio::runtime::Builder;
use tokio::runtime::Runtime;

criterion_group!(benches, signals);

criterion_main!(benches);

struct Settings {
    /// Number of separate machines you want to run the test on.
    /// Set via environment variable `BENCH_NUM_MACHINES`.
    num_machines: u64,
    /// Number of installs to try and reach. You may need
    /// to tweak `BENCH_SAMPLE_SIZE` to actually reach this.
    /// Set via environment variable `BENCH_NUM_INSTALLS`.
    num_installs: usize,
    /// NUmber of conductors to spread installs across.
    /// Set via environment variable `BENCH_NUM_CONDUCTORS`.
    num_conductors: usize,
    /// The url of the local bootstrap to use.
    /// Set via environment variable `BENCH_BOOTSTRAP`.
    url: String,
    /// Happ bundle path for the test that tries to install a happ bundle.
    /// Set via environment variable `BENCH_HAPP`.
    happ_path: PathBuf,
}

impl Settings {
    fn new() -> Self {
        let num_machines = std::env::var_os("BENCH_NUM_MACHINES")
            .and_then(|s| s.to_string_lossy().parse::<u64>().ok())
            .unwrap_or(1);
        let num_installs = std::env::var_os("BENCH_NUM_INSTALLS")
            .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
            .unwrap_or(20);
        let num_conductors = std::env::var_os("BENCH_NUM_CONDUCTORS")
            .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
            .unwrap_or(1);
        let url = std::env::var_os("BENCH_BOOTSTRAP")
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or("http://127.0.0.1:3030".to_string());
        let happ_path = std::env::var_os("BENCH_HAPP")
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or("../../../elemental-chat/elemental-chat.happ".to_string());
        Self {
            num_machines,
            num_installs,
            num_conductors,
            url,
            happ_path: happ_path.into(),
        }
    }
}

fn signals(bench: &mut Criterion) {
    observability::test_run().ok();
    let client = reqwest::Client::new();

    let settings = Settings::new();

    let mut group = bench.benchmark_group("signals");
    group.sample_size(
        std::env::var_os("BENCH_SAMPLE_SIZE")
            .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
            .unwrap_or(100),
    );
    let runtime = rt();

    runtime.block_on(async {
        clear(&client, &settings.url).await;
        sync(&client, settings.num_machines, &settings.url).await;
    });

    let mut producers = runtime.block_on(setup(
        settings.num_conductors,
        settings.num_installs,
        &settings.url,
        settings.happ_path.clone(),
    ));
    group.bench_function(BenchmarkId::new("test", format!("happ")), |b| {
        b.iter(|| {
            runtime.block_on(async { producers.run().await });
        });
    });
    runtime.block_on(async {
        producers
            .consistency(&client, settings.num_machines, &settings.url)
            .await
    });
    group.bench_function(BenchmarkId::new("test", format!("test_wasm")), |b| {
        b.iter(|| {
            runtime.block_on(async { producers.run_test_wasm().await });
        });
    });
    runtime.block_on(async {
        producers
            .consistency(&client, settings.num_machines, &settings.url)
            .await
    });
    group.bench_function(BenchmarkId::new("test", format!("inline_zome")), |b| {
        b.iter(|| {
            runtime.block_on(async { producers.run_inline_zome().await });
        });
    });
    runtime.block_on(async {
        producers
            .consistency(&client, settings.num_machines, &settings.url)
            .await
    });
    runtime.block_on(async move {
        for c in producers.conductors {
            c.shutdown_and_wait().await;
            drop(c);
        }
    });
    runtime.shutdown_background();
}

struct Producers {
    conductors: Vec<SweetConductor>,
    cells: Vec<Vec<SweetCell>>,
    wasm_cells: Vec<Vec<SweetCell>>,
    happ_cells: Vec<Vec<SweetCell>>,
    i: usize,
    total: usize,
    test_dna: DnaFile,
    inline_dna: DnaFile,
    happ_path: PathBuf,
    num_installs: usize,
    signals: tokio::sync::mpsc::Receiver<()>,
    wasm_signals: tokio::sync::mpsc::Receiver<()>,
    signal_message_data: happ_types::SignalMessageData,
}

impl Producers {
    async fn run(&mut self) {
        let start = std::time::Instant::now();
        let mut sent = 0;
        for _ in 0..1 {
            for (i, conductor) in self.conductors.iter_mut().enumerate() {
                for zome in self.happ_cells.get(i).unwrap().iter() {
                    let agents: Vec<AgentPubKey> = self
                        .happ_cells
                        .iter()
                        .flat_map(|c| c.iter().map(|c| c.agent_pubkey().clone()))
                        .collect();
                    sent += agents.len();

                    let _: () = conductor
                        .call(
                            &zome.zome("chat"),
                            "signal_specific_chatters",
                            happ_types::SignalSpecificInput {
                                signal_message_data: self.signal_message_data.clone(),
                                chatters: agents,
                                include_active_chatters: None,
                            },
                        )
                        .await;
                    self.total += 1;
                }
            }
        }
        let mut recv = 0;
        while let Some(_) = self.wasm_signals.recv().await {
            recv += 1;
            if recv >= sent {
                break;
            }
        }
        println!("{}:{:?}", self.total, start.elapsed());
    }

    async fn run_test_wasm(&mut self) {
        let start = std::time::Instant::now();
        let mut sent = 0;
        for _ in 0..1 {
            for (i, conductor) in self.conductors.iter_mut().enumerate() {
                for zome in self.wasm_cells.get(i).unwrap().iter() {
                    let agents: Vec<AgentPubKey> = self
                        .wasm_cells
                        .iter()
                        .flat_map(|c| c.iter().map(|c| c.agent_pubkey().clone()))
                        .collect();
                    sent += agents.len();

                    let _: () = conductor
                        .call(
                            &zome.zome(TestWasm::EmitSignal),
                            "signal_others",
                            RemoteSignal {
                                agents,
                                signal: ExternIO::encode("Hey").unwrap(),
                            },
                        )
                        .await;
                    self.total += 1;
                }
            }
        }
        let mut recv = 0;
        while let Some(_) = self.wasm_signals.recv().await {
            recv += 1;
            if recv >= sent {
                break;
            }
        }
        println!("{}:{:?}", self.total, start.elapsed());
    }

    async fn run_inline_zome(&mut self) {
        let start = std::time::Instant::now();
        let mut sent = 0;
        for _ in 0..1 {
            for (i, conductor) in self.conductors.iter_mut().enumerate() {
                for zome in self.cells.get(i).unwrap().iter() {
                    let agents: Vec<AgentPubKey> = self
                        .cells
                        .iter()
                        .flat_map(|c| c.iter().map(|c| c.agent_pubkey().clone()))
                        .collect();
                    sent += agents.len();

                    let _: () = conductor
                        .call(
                            &zome.zome("app"),
                            "send_signal",
                            RemoteSignal {
                                agents,
                                signal: ExternIO::encode("Hey").unwrap(),
                            },
                        )
                        .await;
                    self.total += 1;
                }
            }
        }
        let mut recv = 0;
        while let Some(_) = self.signals.recv().await {
            recv += 1;
            if recv >= sent {
                break;
            }
        }
        println!("{}:{:?}", self.total, start.elapsed());
    }

    async fn install(&mut self) {
        use holochain_keystore::KeystoreSenderExt;
        for conductor in &mut self.conductors {
            let mut zomes = Vec::new();
            // for _ in 0..self.num_installs {
            //     let agent_key = conductor
            //         .keystore()
            //         .clone()
            //         .generate_sign_keypair_from_pure_entropy()
            //         .await
            //         .expect("Failed to generate agent key");
            //     let agents = vec![agent_key];
            //     let zome = conductor
            //         .setup_app_for_agents("app", &agents, &[self.inline_dna.clone()])
            //         .await
            //         .unwrap()
            //         .cells_flattened()
            //         .pop()
            //         .cloned()
            //         .unwrap();
            //     zomes.push(zome);
            // }
            self.cells.push(zomes);
            let mut zomes = Vec::new();
            for _ in 0..self.num_installs {
                let agent_key = conductor
                    .keystore()
                    .clone()
                    .generate_sign_keypair_from_pure_entropy()
                    .await
                    .expect("Failed to generate agent key");
                let agents = vec![agent_key];
                let zome = conductor
                    .setup_app_for_agents("wasm_app", &agents, &[self.test_dna.clone()])
                    .await
                    .unwrap()
                    .cells_flattened()
                    .pop()
                    .cloned()
                    .unwrap();
                zomes.push(zome);
            }
            self.wasm_cells.push(zomes);
            let mut zomes = Vec::new();
            for i in 0..self.num_installs {
                let agent_key = conductor
                    .keystore()
                    .clone()
                    .generate_sign_keypair_from_pure_entropy()
                    .await
                    .expect("Failed to generate agent key");
                let source = AppBundleSource::Path(self.happ_path.clone());

                let mut membrane_proofs = HashMap::new();
                membrane_proofs.insert(
                    "elemental-chat".to_string(),
                    SerializedBytes::from(UnsafeBytes::from(vec![0])),
                );
                let payload = InstallAppBundlePayload {
                    source,
                    agent_key,
                    installed_app_id: Some(format!("ec {}", i)),
                    membrane_proofs,
                    uid: None,
                };
                let id: InstalledApp =
                    holochain::conductor::handle::ConductorHandleT::install_app_bundle(
                        conductor.inner_handle(),
                        payload,
                    )
                    .await
                    .expect("Failed to install")
                    .into();
                let name = id.id();
                conductor
                    .enable_app(name)
                    .await
                    .expect("Failed to activate app");
                for cell_id in id.all_cells().cloned() {
                    let env = conductor.get_cell_env(&cell_id).await.unwrap();
                    zomes.push(SweetCell::new(cell_id, env));
                }
            }
            self.happ_cells.push(zomes);
        }
    }

    async fn consistency(&mut self, client: &Client, num_machines: u64, url: &str) {
        sync(&client, num_machines, url).await;
        let num_peers = num(client, url).await;
        let mut peers = Vec::new();
        for c in &self.conductors {
            for info in c.get_agent_infos(None).await.unwrap() {
                peers.push(c.get_p2p_env(info.space.clone()).await);
            }
        }
        let peer_refs = peers.iter().collect::<Vec<_>>();
        let mut cells = Vec::new();
        // holochain::test_utils::fixed_peer_consistency_envs_others(
        //     &peer_refs,
        //     num_peers as usize,
        //     2000,
        //     std::time::Duration::from_millis(500),
        // )
        // .await;
        for c in &self.conductors {
            let ids = c
                .list_cell_ids(None)
                .await
                .expect("Failed to list cell ids");
            for id in ids {
                cells.push(c.get_cell_env(&id).await.unwrap());
            }
        }
        let cell_refs = cells.iter().collect::<Vec<_>>();
        // consistency_envs(&cell_refs, 2000, std::time::Duration::from_millis(500)).await;
        holochain::test_utils::fixed_consistency_envs_others(
            &cell_refs,
            num_peers as usize / 2 * 7,
            2000,
            std::time::Duration::from_millis(500),
        )
        .await;
        sync(&client, num_machines, url).await;
    }
}

async fn setup(
    num_conductors: usize,
    num_installs: usize,
    url: &str,
    happ_path: PathBuf,
) -> Producers {
    let config = || {
        let tuning: KitsuneP2pTuningParams = KitsuneP2pTuningParams::default();
        // tuning.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 10;
        // tuning.gossip_peer_on_error_next_gossip_delay_ms = 1000 * 20;

        let mut network = KitsuneP2pConfig::default();
        network.tuning_params = Arc::new(tuning);
        network.bootstrap_service = Some(url2::Url2::parse(url));
        // network.bootstrap_service = Some(url2::url2!("https://bootstrap-staging.holo.host"));
        network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
            bind_to: None,
            override_host: None,
            override_port: None,
        }];
        ConductorConfig {
            network: Some(network),
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            ..Default::default()
        }
    };
    let signal_message_data = happ_types::SignalMessageData {
        message_data: happ_types::MessageData {
            entry: happ_types::Message {
                uuid: "hdsh".to_string(),
                content: "hey".to_string(),
            },
            entry_hash: fixt!(EntryHash),
            created_by: fixt!(AgentPubKey),
            created_at: holochain_types::Timestamp(0, 0),
        },
        channel_data: happ_types::ChannelData {
            entry: happ_types::Channel {
                category: "".to_string(),
                uuid: "".to_string(),
            },
            info: happ_types::ChannelInfo {
                name: "".to_string(),
                created_by: fixt!(AgentPubKey),
                created_at: holochain_types::Timestamp(0, 0),
            },
            latest_chunk: 0,
        },
    };
    let mut configs = Vec::new();
    for _ in 0..num_conductors {
        configs.push(config());
    }
    let conductors = SweetConductorBatch::from_configs(configs.clone()).await;

    let conductors = conductors.into_inner().into_iter().collect::<Vec<_>>();

    let (dna, _) =
        SweetDnaFile::unique_from_test_wasms(vec![holochain_wasm_test_utils::TestWasm::EmitSignal])
            .await
            .unwrap();

    let mut rxs = Vec::new();
    for h in &conductors {
        rxs.push(h.signal_broadcaster().await.subscribe_separately())
    }
    let mut rxs = rxs.into_iter().flatten().collect::<Vec<_>>();

    let (wasm_sender, wasm_signals) = tokio::sync::mpsc::channel(2000);
    tokio::task::spawn(async move {
        loop {
            for rx in &mut rxs {
                let _ = rx.recv().await.unwrap();
                wasm_sender.send(()).await.unwrap()
            }
        }
    });

    let (tx, rx) = tokio::sync::mpsc::channel(2000);

    let inline_zome = holochain_zome_types::InlineZome::new_unique(vec![])
        .callback("init", |api, ()| {
            let mut functions: GrantedFunctions = BTreeSet::new();
            functions.insert((api.zome_info(())?.zome_name, "recv_remote_signal".into()));
            api.create(EntryWithDefId::new(
                EntryDefId::CapGrant,
                Entry::CapGrant(CapGrantEntry {
                    tag: "".into(),
                    // empty access converts to unrestricted
                    access: ().into(),
                    functions,
                }),
            ))?;

            Ok(InitCallbackResult::Pass)
        })
        .callback("send_signal", |api, signal: RemoteSignal| {
            api.remote_signal(signal)?;
            Ok(())
        })
        .callback("recv_remote_signal", {
            move |_api, signal: ExternIO| {
                tx.blocking_send(()).unwrap();
                Ok(())
            }
        });
    let (inline_dna, _) = SweetDnaFile::unique_from_inline_zome("app", inline_zome)
        .await
        .unwrap();
    let mut producers = Producers {
        conductors,
        i: 0,
        total: 0,
        test_dna: dna,
        inline_dna,
        num_installs,
        happ_path,
        cells: Vec::new(),
        wasm_cells: Vec::new(),
        happ_cells: Vec::new(),
        signals: rx,
        wasm_signals,
        signal_message_data,
    };
    producers.install().await;
    producers
}

pub fn rt() -> Runtime {
    Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(24)
        .build()
        .unwrap()
}

async fn clear(client: &Client, url: &str) {
    let res = client
        .post(url)
        .header("X-Op", "clear")
        .header(reqwest::header::CONTENT_TYPE, "application/octet")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
}
async fn num(client: &Client, url: &str) -> u64 {
    let res = client
        .post(url)
        .header("X-Op", "num")
        .header(reqwest::header::CONTENT_TYPE, "application/octet")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let num_peers: u64 =
        kitsune_p2p_types::codec::rmp_decode(&mut res.bytes().await.unwrap().as_ref()).unwrap();
    println!("num_peers {}", num_peers);
    num_peers
}
async fn sync(client: &Client, num_wait_for: u64, url: &str) {
    let mut body_data = Vec::new();
    kitsune_p2p_types::codec::rmp_encode(&mut body_data, num_wait_for).unwrap();
    let res = client
        .post(url)
        .header("X-Op", "sync")
        .header(reqwest::header::CONTENT_TYPE, "application/octet")
        .body(body_data)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
}

mod happ_types {
    //     use crate::{error::ChatError, error::ChatResult, timestamp::Timestamp};
    use hdk::prelude::*;

    // use super::channel::{Channel, ChannelData};

    /// A channel is consists of the category it belongs to
    /// and a unique id
    #[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
    pub struct ChannelInfo {
        pub name: String,
        pub created_by: AgentPubKey,
        pub created_at: Timestamp,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
    pub struct Channel {
        pub category: String,
        pub uuid: String,
    }

    /// The message type that goes to the UI
    #[derive(
        Serialize,
        Deserialize,
        SerializedBytes,
        derive_more::Constructor,
        Debug,
        Clone,
        PartialEq,
        Eq,
    )]
    #[serde(rename_all = "camelCase")]
    pub struct ChannelData {
        pub entry: Channel,
        pub info: ChannelInfo,
        pub latest_chunk: u32,
    }

    /// The actual message data that is saved into the DHT
    #[derive(Debug, Serialize, Deserialize, Clone, SerializedBytes)]
    pub struct Message {
        pub uuid: String,
        pub content: String,
    }

    /// This allows the app to properly order messages.
    /// This message is either the first message of the time block
    /// or has another message that was observed at the time of sending.
    #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
    pub enum LastSeen {
        First,
        Message(EntryHash),
    }

    /// The message type that goes to the UI
    #[derive(Debug, Serialize, Deserialize, Clone, SerializedBytes)]
    #[serde(rename_all = "camelCase")]
    pub struct MessageData {
        pub entry: Message,
        pub entry_hash: EntryHash,
        pub created_by: AgentPubKey,
        pub created_at: Timestamp,
    }

    // Input to the signal_specific_chatters call
    #[derive(Debug, Serialize, Deserialize, SerializedBytes)]
    pub struct SignalSpecificInput {
        pub signal_message_data: SignalMessageData,
        pub chatters: Vec<AgentPubKey>,
        pub include_active_chatters: Option<bool>,
    }

    /// The message type that goes to the UI via emit_signal
    #[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct SignalMessageData {
        pub message_data: MessageData,
        pub channel_data: ChannelData,
    }
}
