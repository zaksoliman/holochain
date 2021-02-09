use futures::StreamExt;
use kitsune_p2p_transport_quic::*;
use kitsune_p2p_types::dependencies::futures;
use kitsune_p2p_types::metrics::*;
use kitsune_p2p_types::transport::*;
use std::sync::Arc;
use std::sync::atomic;
use chrono::prelude::*;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

fn p_(u: u64) {
    if u < 1000 {
        print!("{}", u);
        return;
    }
    p_(u / 1000);
    print!(",{:03}", u % 1000);
}

fn p(t: &str, u: u64) {
    print!("{}", t);
    p_(u);
    println!(" bps");
}

const MSG_SIZE: usize = 512;

fn msg() -> [u8; MSG_SIZE] {
    let now = Utc::now().to_rfc3339();
    let mut out = [0; MSG_SIZE];
    out[0..now.as_bytes().len()].copy_from_slice(now.as_bytes());
    out
}

fn rmsg(m: &[u8]) -> std::time::Duration {
    let mut idx = 0;
    loop {
        if idx >= m.len() || m[idx] == 0 {
            break;
        }
        idx += 1;
    }
    let m = String::from_utf8_lossy(&m[..idx]);
    let m = DateTime::parse_from_rfc3339(&m).unwrap();
    let now = Utc::now();
    let m = now.signed_duration_since(m).to_std().unwrap();
    m
}

const S_CHAN_COUNT: usize = 32;

#[allow(dead_code)]
struct TransportInner {
    con: quinn::Connection,
    s_send: Vec<quinn::SendStream>,
    s_pending: Vec<tokio::sync::oneshot::Sender<quinn::SendStream>>,
}

#[derive(Clone)]
pub struct Transport(ghost::GhostActor<TransportInner>);

pub type TransportRecv = Box<
    dyn futures::stream::Stream<Item = Vec<Box<[u8]>>> + 'static + Send + Unpin
>;

impl Transport {
    pub fn shutdown(&self) {
        self.0.shutdown();
    }

    pub async fn new(
        con: quinn::Connection,
        mut in_uni: quinn::IncomingUniStreams,
    ) -> (Self, TransportRecv) {
        let mut s_send = Vec::new();

        for _ in 0..S_CHAN_COUNT {
            let uni = con.open_uni().await.unwrap();
            let _ = s_send.push(uni);
        }

        let (actor, driver) = ghost::GhostActor::new(TransportInner {
            con,
            s_send,
            s_pending: Vec::new(),
        });

        let (r_send, r_recv) = tokio::sync::mpsc::channel(S_CHAN_COUNT);
        let actor_clone = actor.clone();
        metric_task(async move {
            let limit = Arc::new(tokio::sync::Semaphore::new(S_CHAN_COUNT));
            loop {
                if !actor_clone.is_active() {
                    break;
                }
                let permit = limit.clone().acquire_owned().await;
                match in_uni.next().await {
                    Some(maybe_uni) => {
                        let mut uni = match maybe_uni {
                            Err(e) => {
                                println!("ERR: {:?}", e);
                                continue;
                            }
                            Ok(uni) => uni,
                        };
                        let mut r_send = r_send.clone();
                        let actor_clone = actor_clone.clone();
                        metric_task(async move {
                            let _permit = permit;
                            let mut data = Vec::new();
                            let mut buf = [0_u8; 4096];
                            let mut want_size = None;
                            loop {
                                if !actor_clone.is_active() {
                                    break;
                                }
                                //println!("ABOUT TO READ");
                                match uni.read(&mut buf).await {
                                    Ok(Some(size)) => {
                                        //println!("READ {} bytes", size);
                                        data.extend_from_slice(&buf[0..size]);
                                        loop {
                                            if want_size.is_none() && data.len() >= 8 {
                                                let mut rdr = std::io::Cursor::new(&data[0..8]);
                                                want_size = Some(rdr.read_u64::<LittleEndian>().unwrap());
                                            }
                                            match want_size.clone() {
                                                Some(size) => {
                                                    let size = size as usize;
                                                    if data.len() >= size {
                                                        want_size = None;
                                                        let mut out = data.drain(..size).collect::<Vec<_>>();
                                                        out.drain(..8);

                                                        r_send.send(out.into_boxed_slice()).await.unwrap();
                                                    } else {
                                                        break;
                                                    }
                                                }
                                                None => break,
                                            }
                                        }
                                    }
                                    _ => break,
                                }
                            }
                            println!("INNER READ DROP");
                            TransportResult::Ok(())
                        });
                    }
                    None => break,
                }
            }
            println!("READ DROP");
            TransportResult::Ok(())
        });

        metric_task(async move {
            driver.await;
            TransportResult::Ok(())
        });

        (Self(actor), Box::new(r_recv.ready_chunks(S_CHAN_COUNT)))
    }

    pub async fn send<D: Into<Box<[u8]>>>(&self, d: D) -> TransportResult<()> {
        if !self.0.is_active() {
            return Err("shutdown".into());
        }

        let mut sender: quinn::SendStream = self.0.invoke_async(|inner| {
            //println!("a_s: {}, a_p: {}", inner.s_send.len(), inner.s_pending.len());
            if inner.s_send.is_empty() {
                let (s, r) = tokio::sync::oneshot::channel();
                // TODO - this is unbound - use a semaphore?
                inner.s_pending.push(s);
                Ok(ghost::resp(async move {
                    r.await.map_err(TransportError::other)
                }))
            } else {
                let sender = inner.s_send.remove(0);
                Ok(ghost::resp(async move {
                    Ok(sender)
                }))
            }
        }).await.unwrap();

        static SS: atomic::AtomicU64 = atomic::AtomicU64::new(0);


        // TODO - we need to return the sender even on error here:
        let d = d.into();
        let mut len = Vec::new();
        len.write_u64::<LittleEndian>(d.len() as u64 + 8).unwrap();
        //println!("ABOUT TO SEND");
        sender.write_all(&len).await.map_err(TransportError::other).unwrap();
        //println!("ABOUT TO SEND2");
        sender.write_all(&d).await.map_err(TransportError::other).unwrap();
        SS.fetch_add(512 + 8, atomic::Ordering::Relaxed);
        /*
        println!(
            "actually sent: {}",
            SS.load(atomic::Ordering::Relaxed),
        );
        */

        self.0.invoke(move |inner| {
            if !inner.s_pending.is_empty() {
                let _ = inner.s_pending.remove(0).send(sender);
                return Ok(());
            }
            inner.s_send.push(sender);
            TransportResult::Ok(())
        }).await.unwrap();
        Ok(())
    }
}

#[tokio::test(threaded_scheduler)]
async fn quinn_quic_bench() {
    let mut tx = quinn::TransportConfig::default();

    const EXPECTED_RTT: u64 = 100; // ms
    const MAX_STREAM_BANDWIDTH: u64 = 12500 * 1000; // bytes/s
    const STREAM_RWND: u64 = MAX_STREAM_BANDWIDTH / 1000 * EXPECTED_RTT;

    let w_mult = 1;

    tx.allow_spin(false);
    tx.stream_window_bidi(32 * w_mult);
    tx.stream_receive_window(STREAM_RWND * w_mult);
    tx.receive_window(8 * STREAM_RWND * w_mult);
    tx.send_window(8 * STREAM_RWND * w_mult);
    tx.crypto_buffer_size(16 * 1024 * w_mult as usize);

    let mut options = lair_keystore_api::actor::TlsCertOptions::default();
    options.alg = lair_keystore_api::actor::TlsCertAlg::PkcsEcdsaP256Sha256;
    let cert = lair_keystore_api::internal::tls::tls_cert_self_signed_new_from_entropy(
        options,
    )
    .await
    .unwrap();
    let cert_priv = quinn::PrivateKey::from_der(&cert.priv_key_der).unwrap();
    let cert = quinn::Certificate::from_der(&cert.cert_der).unwrap();

    let mut config = quinn::ServerConfigBuilder::default().build();
    config.certificate(quinn::CertificateChain::from_certs(vec![cert]), cert_priv).unwrap();
    config.transport = Arc::new(tx);
    let addr = tokio::net::lookup_host("127.0.0.1:0").await.unwrap().next().unwrap();

    struct S;
    impl rustls::ServerCertVerifier for S {
        fn verify_server_cert(
            &self,
            _r: &rustls::RootCertStore,
            _p: &[rustls::Certificate],
            _d: webpki::DNSNameRef,
            _o: &[u8],
        ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
            Ok(rustls::ServerCertVerified::assertion())
        }
    }

    let mut cli = quinn::ClientConfigBuilder::default().build();
    cli.transport = config.transport.clone();
    let tls: &mut rustls::ClientConfig = Arc::get_mut(&mut cli.crypto).unwrap();
    tls.dangerous().set_certificate_verifier(Arc::new(S));

    let mut b = quinn::Endpoint::builder();
    b.listen(config.clone());
    b.default_client_config(cli.clone());
    let (e1, _i1) = b.bind(&addr).unwrap();

    let mut b = quinn::Endpoint::builder();
    b.listen(config.clone());
    b.default_client_config(cli.clone());
    let (e2, mut i2) = b.bind(&addr).unwrap();

    let addr = e2.local_addr().unwrap();
    println!("got: {:?}", addr);

    static CONT: atomic::AtomicBool = atomic::AtomicBool::new(true);
    static BPS: atomic::AtomicU64 = atomic::AtomicU64::new(0);
    static LAT: atomic::AtomicU64 = atomic::AtomicU64::new(0);
    let start = std::time::Instant::now();

    metric_task(async move {
        let maybe_con = e1.connect(&addr, "stub.stub").unwrap();
        let quinn::NewConnection {
            connection,
            uni_streams,
            ..
        } = maybe_con.await.unwrap();
        let (con, _recv) = Transport::new(connection, uni_streams).await;

        #[allow(unused_variables)]
        let mut sent = 0_u64;
        loop {
            con.send(msg()).await.unwrap();
            sent += 512 + 8;
            //println!("try sent: {}", sent);
            if !CONT.load(atomic::Ordering::Relaxed) {
                break;
            }
        }
        e1.close(0_u8.into(), b"");
        con.shutdown();

        println!("SENDER_TASK_DONE");
        TransportResult::Ok(())
    });

    metric_task(async move {
        let quinn::NewConnection {
            connection,
            uni_streams,
            ..
        } = i2.next().await.unwrap().await.unwrap();
        let (con, mut recv) = Transport::new(connection, uni_streams).await;

        while let Some(msgs) = recv.next().await {
            for msg in msgs {
                let latency = rmsg(&msg);
                LAT.store(latency.as_micros() as u64, atomic::Ordering::Relaxed);
                BPS.fetch_add(msg.len() as u64 * 8, atomic::Ordering::Relaxed);
                if !CONT.load(atomic::Ordering::Relaxed) {
                    break;
                }
            }
        }
        con.shutdown();

        println!("RECEIVER_TASK_DONE");
        TransportResult::Ok(())
    });

    for _ in 0..5 {
        tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
        let lat = LAT.load(atomic::Ordering::Relaxed);
        let bps = BPS.load(atomic::Ordering::Relaxed);
        let msg = format!("raw quinn (latency: {} us): ", lat);
        p(&msg, bps * 1000 / start.elapsed().as_millis() as u64);
    }

    CONT.store(false, atomic::Ordering::Relaxed);
    e2.close(0_u8.into(), b"");
}

#[tokio::test(threaded_scheduler)]
async fn fake_bench() {
    static CONT: atomic::AtomicBool = atomic::AtomicBool::new(true);
    static BPS: atomic::AtomicU64 = atomic::AtomicU64::new(0);
    static LAT: atomic::AtomicU64 = atomic::AtomicU64::new(0);
    let start = std::time::Instant::now();

    const CONCURRENT: usize = 64;
    let (f_send, f_recv) = tokio::sync::mpsc::channel(CONCURRENT);
    let mut f_recv = f_recv.ready_chunks(CONCURRENT);

    struct Inner {
        f_send: tokio::sync::mpsc::Sender<[u8; 512]>,
    }

    let (actor, driver) = ghost::GhostActor::new(Inner {
        f_send,
    });
    metric_task(async move {
        driver.await;
        TransportResult::Ok(())
    });

    metric_task(async move {
        loop {
            actor.invoke_async(|inner| {
                let mut sender = inner.f_send.clone();
                Ok(ghost::resp(async move {
                    sender.send(msg()).await.unwrap();
                    TransportResult::Ok(())
                }))
            }).await.unwrap();
            if !CONT.load(atomic::Ordering::Relaxed) {
                break;
            }
        }

        println!("SENDER_TASK_DONE");
        TransportResult::Ok(())
    });

    metric_task(async move {
        let limit = Arc::new(tokio::sync::Semaphore::new(CONCURRENT));
        loop {
            let permit = limit.clone().acquire_owned().await;
            let msgs = match f_recv.next().await {
                None => break,
                Some(msgs) => msgs,
            };
            metric_task(async move {
                let _permit = permit;
                for msg in msgs {
                    let latency = rmsg(&msg);
                    LAT.store(latency.as_micros() as u64, atomic::Ordering::Relaxed);
                    BPS.fetch_add(msg.len() as u64 * 8, atomic::Ordering::Relaxed);
                    if !CONT.load(atomic::Ordering::Relaxed) {
                        break;
                    }
                }
                TransportResult::Ok(())
            });
        }

        println!("RECEIVER_TASK_DONE");
        TransportResult::Ok(())
    });

    for _ in 0..5 {
        tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
        let lat = LAT.load(atomic::Ordering::Relaxed);
        let bps = BPS.load(atomic::Ordering::Relaxed);
        let msg = format!("fake bench (latency: {} us): ", lat);
        p(&msg, bps * 1000 / start.elapsed().as_millis() as u64);
    }

    CONT.store(false, atomic::Ordering::Relaxed);
}

#[tokio::test(threaded_scheduler)]
async fn kitsune_transport_quic_bench() {
    let (con1, _evt1) = spawn_transport_listener_quic(
        ConfigListenerQuic::default().set_override_host(Some("127.0.0.1")),
    )
    .await
    .unwrap();

    let (con2, evt2) = spawn_transport_listener_quic(
        ConfigListenerQuic::default().set_override_host(Some("127.0.0.1")),
    )
    .await
    .unwrap();

    static CONT: atomic::AtomicBool = atomic::AtomicBool::new(true);

    const CC: usize = 64;
    metric_task(async move {
        let handle_count = Arc::new(tokio::sync::Semaphore::new(CC));
        evt2.for_each_concurrent(CC, |evt| async {
            match evt {
                TransportEvent::IncomingChannel(_url, mut write, read) => {
                    let permit = handle_count.clone().acquire_owned().await;
                    metric_task(async move {
                        let _permit = permit;
                        let data = read.read_to_end().await;
                        let lat = rmsg(&data);
                        LAT.store(lat.as_micros() as u64, atomic::Ordering::Relaxed);
                        BPS.fetch_add(data.len() as u64 * 8, atomic::Ordering::Relaxed);
                        write.write_and_close(data).await?;
                        TransportResult::Ok(())
                    });
                }
            }
        })
        .await;

        TransportResult::Ok(())
    });

    let bound2 = con2.bound_url().await.unwrap();

    static BPS: atomic::AtomicU64 = atomic::AtomicU64::new(0);
    static LAT: atomic::AtomicU64 = atomic::AtomicU64::new(0);
    let start = std::time::Instant::now();

    metric_task(async move {
        let send_count = Arc::new(tokio::sync::Semaphore::new(CC));
        loop {
            if !CONT.load(atomic::Ordering::Relaxed) {
                break;
            }
            let permit = send_count.clone().acquire_owned().await;
            let fut = con1.request(bound2.clone(), msg().to_vec());
            metric_task(async move {
                let _permit = permit;
                let _ = fut.await?;
                TransportResult::Ok(())
            });
        }
        TransportResult::Ok(())
    });

    for _ in 0..5 {
        tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
        let lat = LAT.load(atomic::Ordering::Relaxed);
        let bps = BPS.load(atomic::Ordering::Relaxed);
        let msg = format!("kitsune quic (latency: {} us): ", lat);
        p(&msg, bps * 1000 / start.elapsed().as_millis() as u64);
    }

    CONT.store(false, atomic::Ordering::Relaxed);
}

#[tokio::test(threaded_scheduler)]
async fn raw_udp_bench() {
    let mut s1 = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    s1.set_multicast_loop_v4(true).unwrap();
    let mut s2 = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    s2.set_multicast_loop_v4(true).unwrap();

    let bound2 = s2.local_addr().unwrap();
    println!("bound to: {:?}", bound2);

    static CONT: atomic::AtomicBool = atomic::AtomicBool::new(true);

    metric_task(async move {
        loop {
            for _ in 0..100 {
                s1.send_to(&msg(), bound2).await.map_err(TransportError::other)?;
            }
            if !CONT.load(atomic::Ordering::Relaxed) {
                break;
            }
        }
        TransportResult::Ok(())
    });

    static BPS: atomic::AtomicU64 = atomic::AtomicU64::new(0);
    let start = std::time::Instant::now();

    metric_task(async move {
        let mut buf: [u8; 512] = [0x00; 512];
        loop {
            let (size, _addr) = s2.recv_from(&mut buf).await.map_err(TransportError::other)?;
            BPS.fetch_add(size as u64 * 8, atomic::Ordering::Relaxed);
            if !CONT.load(atomic::Ordering::Relaxed) {
                break;
            }
        }
        TransportResult::Ok(())
    });

    for _ in 0..5 {
        tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
        let bps = BPS.load(atomic::Ordering::Relaxed);
        p("raw udp: ", bps * 1000 / start.elapsed().as_millis() as u64);
    }

    CONT.store(false, atomic::Ordering::Relaxed);
}
