use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use futures::{sink::SinkExt, stream::StreamExt};
use ghost_actor::dependencies::tracing;
use kitsune_p2p_proxy::*;
use kitsune_p2p_transport_quic::*;
use kitsune_p2p_types::dependencies::spawn_pressure;
use kitsune_p2p_types::metrics::metric_task_warn_limit;
use kitsune_p2p_types::{
    dependencies::{ghost_actor, url2},
    transport::*,
    transport_mem::*,
};
use observability::tracing::Instrument;
use structopt::StructOpt;

/// Proxy transport selector
#[derive(structopt::StructOpt, Debug, Clone, Copy)]
pub enum ProxyTransport {
    /// Use the local in-process memory transport (faster, uses more memory)
    Mem,

    /// Use the real QUIC/UDP transport (slower, more realistic)
    Quic,
}

const E: &str = "please specify 'mem'/'m' or 'quic'/'q'";
impl std::str::FromStr for ProxyTransport {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.chars().next() {
            Some('M') | Some('m') => Ok(Self::Mem),
            Some('Q') | Some('q') => Ok(Self::Quic),
            _ => Err(E),
        }
    }
}

/// Option Parsing
#[derive(structopt::StructOpt, Debug, Clone)]
#[structopt(name = "proxy-stress")]
pub struct Opt {
    /// Transport to test with. ('mem'/'m' or 'quic'/'q')
    #[structopt(short = "t", long, default_value = "Mem")]
    transport: ProxyTransport,

    /// How many client nodes should be spawned
    #[structopt(short = "n", long, default_value = "128")]
    node_count: u32,

    /// Interval between requests per node
    #[structopt(short = "i", long, default_value = "200")]
    request_interval_ms: u32,

    /// Interval between new channels per node
    #[structopt(short, long, default_value = "200")]
    channel_interval_ms: u32,

    /// How long nodes should delay before responding
    #[structopt(short = "d", long, default_value = "200")]
    process_delay_ms: u32,

    /// Grow connections instead of sending messages
    #[structopt(short, long)]
    grow: bool,
}

#[tokio::main]
async fn main() {
    if std::env::var_os("RUST_LOG").is_some() {
        observability::init_fmt(observability::Output::DeadLock)
            .expect("Failed to start contextual logging");
        tokio::spawn(async {
            loop {
                observability::tick_deadlock_catcher();
                tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
            }
        });
    }

    if let Err(e) = inner().await {
        eprintln!("{:?}", e);
    }
}

async fn gen_base_con(
    t: &ProxyTransport,
) -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    futures::channel::mpsc::Receiver<TransportEvent>,
)> {
    match t {
        ProxyTransport::Mem => spawn_bind_transport_mem().await,
        ProxyTransport::Quic => spawn_transport_listener_quic(Default::default()).await,
    }
}

async fn gen_proxy_con(
    t: &ProxyTransport,
) -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    futures::channel::mpsc::Receiver<TransportEvent>,
)> {
    let (listener, events) = gen_base_con(t).await?;
    let proxy_config = ProxyConfig::local_proxy_server(
        TlsConfig::new_ephemeral().await?,
        AcceptProxyCallback::accept_all(),
    );
    spawn_kitsune_proxy_listener(proxy_config, listener, events).await
}

async fn gen_cli_con(
    t: &ProxyTransport,
    proxy_url: url2::Url2,
) -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    futures::channel::mpsc::Receiver<TransportEvent>,
)> {
    let (listener, events) = gen_base_con(t).await?;
    let proxy_config =
        ProxyConfig::remote_proxy_client(TlsConfig::new_ephemeral().await?, proxy_url.into());
    spawn_kitsune_proxy_listener(proxy_config, listener, events).await
}

#[derive(Debug)]
enum Metric {
    Tick,
    RequestOverhead(u64),
}

#[allow(unreachable_code)]
#[allow(clippy::single_match)]
async fn inner() -> TransportResult<()> {
    let opt = Opt::from_args();

    println!("{:#?}", opt);
    kitsune_p2p_types::metrics::init_sys_info_poll();

    let (listener, mut events) = gen_proxy_con(&opt.transport).await?;

    let proxy_url = listener.bound_url().await?;
    println!("Proxy Url: {}", proxy_url);

    metric_task_warn_limit(spawn_pressure::spawn_limit!(10000), async move {
        while let Some(evt) = events.next().await {
            match evt {
                TransportEvent::IncomingChannel(url, mut write, _read) => {
                    tracing::debug!("{} is trying to talk directly to us", url);
                    let _ = write.write_and_close(b"".to_vec()).await;
                }
            }
        }
        <Result<(), ()>>::Ok(())
    });

    let (metric_send, mut metric_recv) =
        futures::channel::mpsc::channel((opt.node_count * 10) as usize);

    metric_task_warn_limit(spawn_pressure::spawn_limit!(10000), {
        let mut metric_send = metric_send.clone();
        async move {
            loop {
                tokio::time::delay_for(std::time::Duration::from_millis(300)).await;
                metric_send
                    .send(Metric::Tick)
                    .await
                    .map_err(TransportError::other)?;
            }
            TransportResult::Ok(())
        }
    });

    metric_task_warn_limit(spawn_pressure::spawn_limit!(10000), async move {
        let mut last_disp = std::time::Instant::now();
        let mut rtime = Vec::new();
        while let Some(metric) = metric_recv.next().await {
            match metric {
                Metric::RequestOverhead(time) => rtime.push(time),
                _ => (),
            }
            if last_disp.elapsed().as_millis() > 5000 {
                dbg!(rtime.len());
                last_disp = std::time::Instant::now();
                let cnt = rtime.len() as f64;
                let mut avg = rtime.drain(..).fold(0.0_f64, |acc, x| acc + (x as f64));
                avg /= cnt;
                println!("Avg Request Overhead ({} requests): {} ms", cnt, avg);
            }
        }
        <Result<(), ()>>::Ok(())
    });

    let (_con, con_url) = gen_client(opt.clone(), proxy_url.clone()).await?;
    println!("Responder Url: {}", con_url);

    let mut num = Arc::new(AtomicUsize::new(0));
    let t = Arc::new(std::time::Instant::now());
    for _ in 0..opt.node_count {
        if opt.grow {
            metric_task_warn_limit(
                spawn_pressure::spawn_limit!(10000),
                client_loop_grow(
                    opt.clone(),
                    proxy_url.clone(),
                    con_url.clone(),
                    metric_send.clone(),
                    num.clone(),
                    t.clone(),
                ),
            );
        } else {
            metric_task_warn_limit(
                spawn_pressure::spawn_limit!(10000),
                client_loop(
                    opt.clone(),
                    proxy_url.clone(),
                    con_url.clone(),
                    metric_send.clone(),
                ),
            );
        }
    }

    // wait for ctrl-c
    futures::future::pending().await
}

async fn gen_client(
    opt: Opt,
    proxy_url: url2::Url2,
) -> TransportResult<(ghost_actor::GhostSender<TransportListener>, url2::Url2)> {
    let (con, events) = gen_cli_con(&opt.transport, proxy_url).await?;

    let con_url = con.bound_url().await?;

    metric_task_warn_limit(spawn_pressure::spawn_limit!(10000), async move {
        let process_delay_ms = opt.process_delay_ms as u64;
        events
            .for_each_concurrent(
                /* limit */ (opt.node_count + 10) as usize,
                move |evt| {
                    async move {
                        match evt {
                            TransportEvent::IncomingChannel(_url, mut write, read) => {
                                tokio::time::delay_for(std::time::Duration::from_millis(
                                    process_delay_ms,
                                ))
                                .await;
                                let data = read.read_to_end().await;
                                let data = format!("echo: {}", String::from_utf8_lossy(&data))
                                    .into_bytes();
                                let _ = write
                                    .write_and_close(data)
                                    .instrument(tracing::debug_span!("responder"))
                                    .await;
                            }
                        }
                    }
                    .instrument(tracing::debug_span!("resp_loop_inner"))
                },
            )
            .await;
        <Result<(), ()>>::Ok(())
    });

    Ok((con, con_url))
}

async fn client_loop(
    opt: Opt,
    proxy_url: url2::Url2,
    con_url: url2::Url2,
    mut metric_send: futures::channel::mpsc::Sender<Metric>,
) -> TransportResult<()> {
    let (con, _my_url) = gen_client(opt.clone(), proxy_url).await?;

    loop {
        tokio::time::delay_for(std::time::Duration::from_millis(
            opt.request_interval_ms as u64,
        ))
        .await;

        let start = std::time::Instant::now();
        let (_, mut write, read) = con.create_channel(con_url.clone()).await?;
        write.write_and_close(b"".to_vec()).await?;
        read.read_to_end().await;
        metric_send
            .send(Metric::RequestOverhead(
                start.elapsed().as_millis() as u64 - opt.process_delay_ms as u64,
            ))
            .await
            .map_err(TransportError::other)?;
    }
}

async fn client_loop_grow(
    opt: Opt,
    proxy_url: url2::Url2,
    con_url: url2::Url2,
    metric_send: futures::channel::mpsc::Sender<Metric>,
    num: Arc<AtomicUsize>,
    t: Arc<std::time::Instant>,
) -> TransportResult<()> {
    let (con, _my_url) = gen_client(opt.clone(), proxy_url).await?;

    metric_task_warn_limit::<(), (), _>(spawn_pressure::spawn_limit!(1000), {
        let con = con.clone();
        async move {
            loop {
                tokio::time::delay_for(std::time::Duration::from_secs(4)).await;
                let d = con.debug().await?;
                println!("{}", d);
            }
        }
    });
    let size = b"hello".to_vec().len();
    loop {
        tokio::time::delay_for(std::time::Duration::from_millis(
            opt.channel_interval_ms as u64,
        ))
        .await;

        let request_interval_ms = opt.request_interval_ms;
        let process_delay_ms = opt.process_delay_ms;

        let start = std::time::Instant::now();
        let channel = con
            .create_channel(con_url.clone())
            .instrument(tracing::debug_span!("create_channel_from_test"))
            .await?;
        metric_task_warn_limit(spawn_pressure::spawn_limit!(1000), {
            let mut metric_send = metric_send.clone();
            let num = num.clone();
            let t = t.clone();
            async move {
                tokio::time::delay_for(std::time::Duration::from_millis(
                    request_interval_ms as u64,
                ))
                .await;
                let (_, mut write, read) = channel;
                let num = num.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                write
                    .write_and_close(b"hello".to_vec())
                    .instrument(tracing::debug_span!("writing_to_channel"))
                    .await?;
                let resp = read
                    .read_to_end()
                    .instrument(tracing::debug_span!("reading_from_channel"))
                    .await;
                let size = resp.len() + size * 8;
                let el = t.elapsed();
                let avg: std::time::Duration = el / num as u32;
                let latency = start.elapsed();
                let mps = num.checked_div(el.as_secs() as usize).unwrap_or(0);
                let mut mbps = (size * num as usize)
                    .checked_div(el.as_secs() as usize)
                    .unwrap_or(0);
                mbps /= 1000;

                println!(
                    "messages per s: {}, {}Mbps, latency {:?}, avg latency {:?}, total {}, el {}",
                    mps,
                    mbps,
                    latency,
                    avg,
                    num,
                    el.as_secs()
                );
                // metric_send
                //     .send(Metric::RequestOverhead(
                //         (start.elapsed().as_millis() as u64)
                //             .checked_sub(process_delay_ms as u64)
                //             .unwrap_or_else(|| {
                //                 dbg!("FAIL");
                //                 0
                //             }),
                //     ))
                //     .await
                //     .map_err(TransportError::other)?;
                <Result<(), ()>>::Ok(())
            }
            .instrument(tracing::debug_span!("sender_outer"))
        })
        .await
        .unwrap()
        .unwrap();
        // let d = con.debug().await?;
        // println!("{}", d);
    }
}
