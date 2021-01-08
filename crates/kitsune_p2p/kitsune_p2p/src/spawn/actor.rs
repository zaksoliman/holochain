// this is largely a passthrough that routes to a specific space handler

use crate::actor;
use crate::actor::*;
use crate::event::*;
use crate::gossip::*;
use crate::*;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use kitsune_p2p_types::async_lazy::AsyncLazy;
use kitsune_p2p_types::transport::*;
use kitsune_p2p_types::transport_pool::*;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// The bootstrap service is much more thoroughly documented in the default service implementation.
/// @see https://github.com/holochain/bootstrap
mod bootstrap;
mod discover;
mod gossip;
mod space;
use ghost_actor::dependencies::must_future;
use ghost_actor::dependencies::tracing;
use space::*;

ghost_actor::ghost_chan! {
    pub(crate) chan Internal<crate::KitsuneP2pError> {
        /// Register space event handler
        fn register_space_event_handler(recv: futures::channel::mpsc::Receiver<KitsuneP2pEvent>) -> ();
    }
}

pub(crate) struct KitsuneP2pActor {
    channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
    internal_sender: ghost_actor::GhostSender<Internal>,
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    transport: ghost_actor::GhostSender<TransportListener>,
    spaces: HashMap<Arc<KitsuneSpace>, AsyncLazy<ghost_actor::GhostSender<KitsuneP2p>>>,
    config: Arc<KitsuneP2pConfig>,
}

fn build_transport(
    t_conf: TransportConfig,
    tls_config: Arc<kitsune_p2p_proxy::TlsConfig>,
) -> must_future::MustBoxFuture<
    'static,
    TransportResult<(
        ghost_actor::GhostSender<TransportListener>,
        TransportEventReceiver,
    )>,
> {
    must_future::MustBoxFuture::new(async move {
        match t_conf {
            TransportConfig::Mem {} => {
                Ok(kitsune_p2p_types::transport_mem::spawn_bind_transport_mem().await?)
            }
            TransportConfig::Quic {
                bind_to,
                override_host,
                override_port,
            } => {
                let sub_conf = kitsune_p2p_transport_quic::ConfigListenerQuic::default()
                    .set_bind_to(bind_to)
                    .set_override_host(override_host)
                    .set_override_port(override_port);
                Ok(kitsune_p2p_transport_quic::spawn_transport_listener_quic(sub_conf).await?)
            }
            TransportConfig::Proxy {
                sub_transport,
                proxy_config,
            } => {
                let (sub_lstn, sub_evt) =
                    build_transport(*sub_transport, tls_config.clone()).await?;
                let sub_conf = match proxy_config {
                    ProxyConfig::RemoteProxyClient { proxy_url } => {
                        kitsune_p2p_proxy::ProxyConfig::remote_proxy_client(
                            (*tls_config).clone(),
                            proxy_url.into(),
                        )
                    }
                    ProxyConfig::LocalProxyServer {
                        proxy_accept_config,
                    } => kitsune_p2p_proxy::ProxyConfig::local_proxy_server(
                        (*tls_config).clone(),
                        match proxy_accept_config {
                            Some(ProxyAcceptConfig::AcceptAll) => {
                                kitsune_p2p_proxy::AcceptProxyCallback::accept_all()
                            }
                            None | Some(ProxyAcceptConfig::RejectAll) => {
                                kitsune_p2p_proxy::AcceptProxyCallback::reject_all()
                            }
                        },
                    ),
                };
                Ok(
                    kitsune_p2p_proxy::spawn_kitsune_proxy_listener(sub_conf, sub_lstn, sub_evt)
                        .await?,
                )
            }
        }
    })
}

impl KitsuneP2pActor {
    pub async fn new(
        config: KitsuneP2pConfig,
        tls_config: kitsune_p2p_proxy::TlsConfig,
        channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
        internal_sender: ghost_actor::GhostSender<Internal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ) -> KitsuneP2pResult<Self> {
        let tls_config = Arc::new(tls_config);
        let (t_pool, transport, t_event) = spawn_transport_pool().await?;
        for t_conf in config.transport_pool.clone() {
            let (l, e) = build_transport(t_conf, tls_config.clone()).await?;
            t_pool.push_sub_transport(l, e).await?;
        }

        tokio::task::spawn({
            let nanoid = nanoid::nanoid!();
            let names = [
                "call",
                "notify",
                "gossip",
                "agent_info",
                "fetch_hash",
                "fetch_data",
                "failure",
            ];
            let other_names = ["gossip_ops", "gossip_agents"];
            let stats_in: HashMap<_, _> = names
                .iter()
                .map(|&name| (name, Arc::new((AtomicU64::new(0), AtomicU64::new(0)))))
                .collect();
            let stats_out: HashMap<_, _> = names
                .iter()
                .map(|&name| (name, Arc::new((AtomicU64::new(0), AtomicU64::new(0)))))
                .collect();
            let stats_other: HashMap<_, _> = other_names
                .iter()
                .map(|&name| (name, Arc::new((AtomicU64::new(0), AtomicU64::new(0)))))
                .collect();

            let evt_sender = evt_sender.clone();
            t_event.for_each_concurrent(/* limit */ 10, move |event| {
                let evt_sender = evt_sender.clone();
                let stats_in = stats_in.clone();
                let stats_out = stats_out.clone();
                let stats_other = stats_other.clone();
                let nanoid = nanoid.clone();
                async move {
                    let span = tracing::debug_span!("recv_stat");
                    let mut total_bytes = 0.0;
                    let mut total_calls = 0;
                    span.in_scope(|| {
                        for (stat, v) in &stats_in {
                            let bytes = v.0.load(std::sync::atomic::Ordering::SeqCst) as f64
                                / 1_000_000.0;
                            let calls = v.1.load(std::sync::atomic::Ordering::SeqCst);
                            tracing::debug!(?stat, recv_bytes = ?bytes, ?calls);
                            total_bytes += bytes;
                            total_calls += calls;
                        }
                        tracing::debug!(?nanoid, total_recv_bytes = ?total_bytes, total_recv_calls = ?total_calls);
                    });
                    let mut total_bytes = 0.0;
                    let mut total_calls = 0;
                    span.in_scope(|| {
                        for (stat, v) in &stats_out {
                            let bytes = v.0.load(std::sync::atomic::Ordering::SeqCst) as f64
                                / 1_000_000.0;
                            let calls = v.1.load(std::sync::atomic::Ordering::SeqCst);
                            tracing::debug!(?stat, sent_bytes = ?bytes, ?calls);
                            total_bytes += bytes;
                            total_calls += calls;
                        }
                        let tls_overhead = (500 * total_calls) as f64 / 1_000_000.0;
                        tracing::debug!(?nanoid, total_send_bytes = ?total_bytes, total_send_calls = ?total_calls, ?tls_overhead);
                    });
                    let mut total_a = 0;
                    let mut total_b = 0;
                    span.in_scope(|| {
                        for (stat, v) in &stats_other {
                            let a = v.0.load(std::sync::atomic::Ordering::SeqCst);
                            let b = v.1.load(std::sync::atomic::Ordering::SeqCst);
                            tracing::debug!(?stat, ?a, ?b);
                            total_a += a;
                            total_b += b;
                        }
                        tracing::debug!(?nanoid, ?total_a, ?total_b);
                    });

                    let evt_sender = &evt_sender;
                    match event {
                        TransportEvent::IncomingChannel(_url, mut write, read) => {
                            let read = read.read_to_end().await;
                            let stat_size = read.len() as u64;
                            use kitsune_p2p_types::codec::Codec;
                            let read = match wire::Wire::decode_ref(&read) {
                                Err(err) => {
                                    let reason = format!("{:?}", err);
                                    let fail = wire::Wire::failure(reason).encode_vec().unwrap();
                                    let _ = write.write_and_close(fail).await;
                                    return;
                                }
                                Ok((_, r)) => r,
                            };
                            match read {
                                wire::Wire::Call(wire::Call {
                                    space,
                                    from_agent,
                                    to_agent,
                                    data,
                                    ..
                                }) => {
                                    stats_in.get("call").unwrap().0
                                        .fetch_add(stat_size, std::sync::atomic::Ordering::SeqCst);
                                    let res = match evt_sender
                                        .call(space, to_agent, from_agent, data.into())
                                        .await
                                    {
                                        Err(err) => {
                                            let reason = format!("{:?}", err);
                                            let fail =
                                                wire::Wire::failure(reason).encode_vec().unwrap();
                                            let _ = write.write_and_close(fail).await;
                                            return;
                                        }
                                        Ok(r) => r,
                                    };
                                    let resp =
                                        wire::Wire::call_resp(res.into()).encode_vec().unwrap();
                                    stats_out.get("call").unwrap().0.fetch_add(
                                        resp.len() as u64,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    stats_out.get("call").unwrap().1.fetch_add(
                                        1,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    let _ = write.write_and_close(resp).await;
                                }
                                wire::Wire::Notify(wire::Notify {
                                    space,
                                    from_agent,
                                    to_agent,
                                    data,
                                    ..
                                }) => {
                                    stats_in.get("notify").unwrap().0
                                        .fetch_add(stat_size, std::sync::atomic::Ordering::SeqCst);
                                    if let Err(err) = evt_sender
                                        .notify(space, to_agent, from_agent, data.into())
                                        .await
                                    {
                                        let reason = format!("{:?}", err);
                                        let fail =
                                            wire::Wire::failure(reason).encode_vec().unwrap();
                                        let _ = write.write_and_close(fail).await;
                                        return;
                                    }
                                    let resp = wire::Wire::notify_resp().encode_vec().unwrap();
                                    stats_out.get("notify").unwrap().0.fetch_add(
                                        resp.len() as u64,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    stats_out.get("notify").unwrap().1.fetch_add(
                                        1,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    let _ = write.write_and_close(resp).await;
                                }
                                wire::Wire::FetchOpHashes(wire::FetchOpHashes {
                                    space,
                                    from_agent,
                                    to_agent,
                                    dht_arc,
                                    since_utc_epoch_s,
                                    until_utc_epoch_s,
                                    last_count,
                                }) => {
                                    stats_in.get("fetch_hash").unwrap().0
                                        .fetch_add(stat_size, std::sync::atomic::Ordering::SeqCst);
                                    let input = ReqOpHashesEvt::new(
                                        from_agent,
                                        to_agent,
                                        dht_arc,
                                        since_utc_epoch_s,
                                        until_utc_epoch_s,
                                    );
                                    let (hashes, agent_hashes) = match local_req_op_hashes(
                                        &evt_sender,
                                        space,
                                        input,
                                    )
                                    .await
                                    {
                                        Err(err) => {
                                            let reason = format!("{:?}", err);
                                            let fail =
                                                wire::Wire::failure(reason).encode_vec().unwrap();
                                            let _ = write.write_and_close(fail).await;
                                            return;
                                        }
                                        Ok(r) => r,
                                    };
                                    let s = tracing::debug_span!("new_ops");
                                    let hashes = if last_count == hashes.len() as u64 {
                                        s.in_scope(|| tracing::debug!("No Change"));
                                        NewOps::NoChange
                                    } else {
                                        s.in_scope(|| tracing::debug!(new_hashes = ?hashes.len()));
                                        NewOps::New(hashes)
                                    };
                                    let resp =
                                        wire::Wire::fetch_op_hashes_response(hashes, agent_hashes)
                                            .encode_vec()
                                            .expect("This encoding should never fail");
                                    stats_out.get("fetch_hash").unwrap().0
                                    .fetch_add(
                                        resp.len() as u64,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    stats_out.get("fetch_hash").unwrap().1.fetch_add(
                                        1,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    let _ = write.write_and_close(resp).await;
                                }
                                wire::Wire::FetchOpData(wire::FetchOpData {
                                    space,
                                    from_agent,
                                    to_agent,
                                    op_hashes,
                                    peer_hashes,
                                }) => {
                                    stats_in.get("fetch_data").unwrap().0
                                        .fetch_add(stat_size, std::sync::atomic::Ordering::SeqCst);
                                    let input = ReqOpDataEvt::new(
                                        from_agent,
                                        to_agent,
                                        op_hashes,
                                        peer_hashes,
                                    );
                                    let (op_data, agent_infos) =
                                        match local_req_op_data(&evt_sender, space, input).await {
                                            Err(err) => {
                                                let reason = format!("{:?}", err);
                                                let fail = wire::Wire::failure(reason)
                                                    .encode_vec()
                                                    .unwrap();
                                                let _ = write.write_and_close(fail).await;
                                                return;
                                            }
                                            Ok(r) => r,
                                        };
                                    let op_data =
                                        op_data.into_iter().map(|(h, op)| (h, op.into())).collect();
                                    let resp =
                                        wire::Wire::fetch_op_data_response(op_data, agent_infos)
                                            .encode_vec()
                                            .expect("This encoding should never fail");
                                    stats_out.get("fetch_data").unwrap().0
                                    .fetch_add(
                                        resp.len() as u64,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    stats_out.get("fetch_data").unwrap().1.fetch_add(
                                        1,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    let _ = write.write_and_close(resp).await;
                                }
                                wire::Wire::AgentInfoQuery(q) => {
                                    stats_in.get("agent_info").unwrap().0
                                        .fetch_add(stat_size, std::sync::atomic::Ordering::SeqCst);
                                    match agent_info_query(q, evt_sender.clone()).await {
                                        Ok(r) => {
                                            let resp = wire::Wire::agent_info_query_resp(r)
                                                .encode_vec()
                                                .unwrap();
                                            stats_out.get("agent_info").unwrap().0
                                            .fetch_add(
                                                resp.len() as u64,
                                                std::sync::atomic::Ordering::SeqCst,
                                            );
                                            stats_out.get("agent_info").unwrap().1.fetch_add(
                                                1,
                                                std::sync::atomic::Ordering::SeqCst,
                                            );
                                            let _ = write.write_and_close(resp).await;
                                        }
                                        Err(err) => {
                                            let reason = format!("{:?}", err);
                                            let fail =
                                                wire::Wire::failure(reason).encode_vec().unwrap();
                                            let _ = write.write_and_close(fail).await;
                                        }
                                    }
                                }
                                wire::Wire::Gossip(wire::Gossip {
                                    space,
                                    from_agent,
                                    to_agent,
                                    ops,
                                    agents,
                                }) => {
                                    // dbg!(stat_size);
                                    // dbg!(ops.len());
                                    // dbg!(agents.len());
                                    stats_in.get("gossip").unwrap().0
                                        .fetch_add(stat_size, std::sync::atomic::Ordering::SeqCst);
                                    stats_other.get("gossip_ops").unwrap().0.fetch_add(
                                        ops.len() as u64,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    stats_other.get("gossip_agents").unwrap().0
                                    .fetch_add(
                                        agents.len() as u64,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    let input = GossipEvt::new(
                                        from_agent,
                                        to_agent,
                                        ops.into_iter().map(|(k, v)| (k, v.into())).collect(),
                                        agents,
                                    );
                                    if let Err(err) =
                                        local_gossip_ops(&evt_sender, space, input).await
                                    {
                                        let reason = format!("{:?}", err);
                                        tracing::error!("got err: {}", reason);
                                        let fail =
                                            wire::Wire::failure(reason).encode_vec().unwrap();
                                        let _ = write.write_and_close(fail).await;
                                        return;
                                    }
                                    let resp = wire::Wire::gossip_resp().encode_vec().unwrap();
                                    stats_out.get("gossip").unwrap().0
                                    .fetch_add(
                                        resp.len() as u64,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    stats_out.get("gossip").unwrap().1.fetch_add(
                                        1,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    let _ = write.write_and_close(resp).await;
                                }
                                _ => unimplemented!("{:?}", read),
                            }
                        }
                    }
                }
            })
        });

        Ok(Self {
            channel_factory,
            internal_sender,
            evt_sender,
            transport,
            spaces: HashMap::new(),
            config: Arc::new(config),
        })
    }
}

async fn agent_info_query(
    q: wire::AgentInfoQuery,
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
) -> Result<Vec<crate::types::agent_store::AgentInfoSigned>, KitsuneP2pError> {
    let wire::AgentInfoQuery {
        space,
        to_agent,
        by_agent,
        by_basis_arc,
    } = q;

    if let Some(by_agent) = by_agent {
        if let Some(agent) = evt_sender
            .get_agent_info_signed(GetAgentInfoSignedEvt {
                space,
                agent: by_agent,
            })
            .await?
        {
            Ok(vec![agent])
        } else {
            Ok(vec![])
        }
    } else if let Some(_by_basis_arc) = by_basis_arc {
        Ok(evt_sender
            .query_agent_info_signed(QueryAgentInfoSignedEvt {
                space,
                agent: to_agent,
            })
            .await?)
    } else {
        Err("must specify by_agent or by_basis_arc".into())
    }
}

impl ghost_actor::GhostControlHandler for KitsuneP2pActor {}

impl ghost_actor::GhostHandler<Internal> for KitsuneP2pActor {}

impl InternalHandler for KitsuneP2pActor {
    fn handle_register_space_event_handler(
        &mut self,
        recv: futures::channel::mpsc::Receiver<KitsuneP2pEvent>,
    ) -> InternalHandlerResult<()> {
        let f = self.channel_factory.attach_receiver(recv);
        Ok(async move {
            f.await?;
            Ok(())
        }
        .boxed()
        .into())
    }
}

impl ghost_actor::GhostHandler<KitsuneP2pEvent> for KitsuneP2pActor {}

impl KitsuneP2pEventHandler for KitsuneP2pActor {
    fn handle_put_agent_info_signed(
        &mut self,
        input: crate::event::PutAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self.evt_sender.put_agent_info_signed(input))
    }

    fn handle_get_agent_info_signed(
        &mut self,
        input: crate::event::GetAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        Ok(self.evt_sender.get_agent_info_signed(input))
    }

    fn handle_query_agent_info_signed(
        &mut self,
        input: crate::event::QueryAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<crate::types::agent_store::AgentInfoSigned>> {
        Ok(self.evt_sender.query_agent_info_signed(input))
    }

    fn handle_call(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        Ok(self.evt_sender.call(space, to_agent, from_agent, payload))
    }

    fn handle_notify(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self.evt_sender.notify(space, to_agent, from_agent, payload))
    }

    fn handle_gossip(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        op_hash: Arc<KitsuneOpHash>,
        op_data: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        Ok(self
            .evt_sender
            .gossip(space, to_agent, from_agent, op_hash, op_data))
    }

    fn handle_fetch_op_hashes_for_constraints(
        &mut self,
        input: FetchOpHashesForConstraintsEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<Arc<KitsuneOpHash>>> {
        Ok(self.evt_sender.fetch_op_hashes_for_constraints(input))
    }

    fn handle_fetch_op_hash_data(
        &mut self,
        input: FetchOpHashDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
        Ok(self.evt_sender.fetch_op_hash_data(input))
    }

    fn handle_sign_network_data(
        &mut self,
        input: SignNetworkDataEvt,
    ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> {
        Ok(self.evt_sender.sign_network_data(input))
    }
}

impl ghost_actor::GhostHandler<KitsuneP2p> for KitsuneP2pActor {}

impl KitsuneP2pHandler for KitsuneP2pActor {
    fn handle_list_transport_bindings(&mut self) -> KitsuneP2pHandlerResult<Vec<url2::Url2>> {
        let fut = self.transport.bound_url();
        Ok(async move {
            let urls = fut.await?;
            Ok(urls
                .query_pairs()
                .map(|(_, url)| url2::url2!("{}", url))
                .collect())
        }
        .boxed()
        .into())
    }

    fn handle_join(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let internal_sender = self.internal_sender.clone();
        let space2 = space.clone();
        let transport = self.transport.clone();
        let config = Arc::clone(&self.config);
        let space_sender = match self.spaces.entry(space.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(AsyncLazy::new(async move {
                let (send, evt_recv) = spawn_space(space2, transport, config)
                    .await
                    .expect("cannot fail to create space");
                internal_sender
                    .register_space_event_handler(evt_recv)
                    .await
                    .expect("FAIL");
                send
            })),
        };
        let space_sender = space_sender.get();
        Ok(async move { space_sender.await.join(space, agent).await }
            .boxed()
            .into())
    }

    fn handle_leave(
        &mut self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
    ) -> KitsuneP2pHandlerResult<()> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Ok(async move { Ok(()) }.boxed().into()),
            Some(space) => space.get(),
        };
        Ok(async move {
            space_sender.await.leave(space.clone(), agent).await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_rpc_single(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
        timeout_ms: Option<u64>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        let space_sender = match self.spaces.get_mut(&space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(space)),
            Some(space) => space.get(),
        };
        Ok(async move {
            space_sender
                .await
                .rpc_single(space, to_agent, from_agent, payload, timeout_ms)
                .await
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self, input))]
    fn handle_rpc_multi(
        &mut self,
        input: actor::RpcMulti,
    ) -> KitsuneP2pHandlerResult<Vec<actor::RpcMultiResponse>> {
        let space_sender = match self.spaces.get_mut(&input.space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(input.space)),
            Some(space) => space.get(),
        };
        Ok(async move { space_sender.await.rpc_multi(input).await }
            .boxed()
            .into())
    }

    fn handle_notify_multi(&mut self, input: actor::NotifyMulti) -> KitsuneP2pHandlerResult<u8> {
        let space_sender = match self.spaces.get_mut(&input.space) {
            None => return Err(KitsuneP2pError::RoutingSpaceError(input.space)),
            Some(space) => space.get(),
        };
        Ok(async move { space_sender.await.notify_multi(input).await }
            .boxed()
            .into())
    }
}
