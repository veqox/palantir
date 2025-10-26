#![feature(ip)]

mod event;
mod resolver;

use std::{
    collections::{HashMap, hash_map::Entry},
    convert::Infallible,
    env,
    mem::zeroed,
    net::IpAddr,
    os::fd::AsRawFd,
    sync::Arc,
    time::{Duration, SystemTime},
};

use async_stream::stream;
use axum::{
    Router,
    extract::State,
    response::{Sse, sse},
    routing::get,
};
use aya::{
    Ebpf,
    maps::RingBuf,
    programs::{SchedClassifier, TcAttachType},
};
use futures_util::Stream;
use libc::{CLOCK_BOOTTIME, CLOCK_REALTIME, clock_gettime, timespec};
use palantir_ebpf_common::{self, Direction, RawEvent};
use tokio::{
    io::{Interest, unix::AsyncFd},
    net::TcpListener,
    sync::{Mutex, broadcast},
};
use tower_http::{
    cors::{self, CorsLayer},
    trace::TraceLayer,
};
use tracing::{trace, warn};
use tracing_subscriber::EnvFilter;

use crate::{
    event::{Event, Packet, Peer},
    resolver::{IpInfo, Resolver},
};

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<Event>,
    peers: Arc<Mutex<HashMap<IpAddr, Peer>>>,
}

#[tokio::main]
async fn main() {
    let server_peer = Peer {
        addr: env::var("SERVER_ADDR")
            .expect("SERVER_ADDR is not defined")
            .parse()
            .expect("SERVER_ADDR is not a valid IpAddr"),
        info: IpInfo {
            lat: env::var("SERVER_LAT")
                .expect("SERVER_LAT is not defined")
                .parse()
                .expect("SERVER_LAT is not a valid f64"),
            lon: env::var("SERVER_LON")
                .expect("SERVER_LON is not defined")
                .parse()
                .expect("SERVER_LON is not a valid f64"),
            country_code: env::var("SERVER_COUNTRY_CODE")
                .expect("SERVER_COUNTRY_CODE is not defined"),
            details: resolver::LocationDetails::Manual,
        },
        ingress_bytes: 0,
        egress_bytes: 0,
        last_message: None,
    };

    let iface = env::var("IFACE").expect("IFACE is not defined");

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let boot_time = unsafe {
        let mut ts: timespec = zeroed();
        clock_gettime(CLOCK_BOOTTIME, &mut ts);
        Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
    };

    let real_time = unsafe {
        let mut ts: timespec = zeroed();
        clock_gettime(CLOCK_REALTIME, &mut ts);
        Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
    };

    let boot_time = SystemTime::UNIX_EPOCH + (real_time - boot_time);

    let (tx, _) = broadcast::channel(64);

    let state = Arc::new(AppState {
        tx: tx.clone(),
        peers: Arc::new(Mutex::new(HashMap::from_iter([(
            server_peer.addr,
            server_peer,
        )]))),
    });

    tokio::spawn({
        let tx = tx.clone();
        let state = state.clone();

        let city_reader =
            maxminddb::Reader::from_source(include_bytes!("../../../assets/GeoLite2-City.mmdb"))
                .expect("failed to initialize reader");
        let resolver = Resolver::new(city_reader);

        async move {
            let mut cache = HashMap::<IpAddr, IpInfo>::new();
            let mut ebpf = init_ebpf(&iface);

            let mut events = RingBuf::try_from(ebpf.map_mut("EVENTS").unwrap()).unwrap();

            let poll = AsyncFd::new(events.as_raw_fd()).unwrap();
            loop {
                let mut guard = poll.readable().await.unwrap();
                while let Some(item) = events.next() {
                    let raw_event = unsafe { *(item.as_ptr() as *const RawEvent) };
                    let peer_addr = raw_event.peer_addr();

                    if peer_addr.is_multicast() || !peer_addr.is_global() {
                        continue;
                    }

                    trace!("{:?}", raw_event);

                    let peer_info = match cache.entry(peer_addr) {
                        Entry::Occupied(entry) => entry.get().clone(),
                        Entry::Vacant(entry) => match resolver.resolve(peer_addr) {
                            Some(info) => entry.insert(info).clone(),
                            None => {
                                warn!("failed to get ip info for {}, skipping", peer_addr);
                                continue;
                            }
                        },
                    };

                    {
                        let mut peers = state.peers.lock().await;

                        let mut is_new = false;

                        let peer = peers.entry(peer_addr).or_insert_with(|| {
                            is_new = true;
                            Peer {
                                addr: peer_addr,
                                ingress_bytes: 0,
                                egress_bytes: 0,
                                last_message: None,
                                info: peer_info,
                            }
                        });

                        let bytes = raw_event.bytes as u64;
                        match raw_event.direction {
                            Direction::Ingress => peer.ingress_bytes += bytes,
                            Direction::Egress => peer.egress_bytes += bytes,
                        }

                        peer.last_message = Some(raw_event.timestamp(boot_time));

                        if is_new {
                            let _ = tx.send(Event::Peer(peer.clone()));
                        }
                    }

                    {
                        let mut peers = state.peers.lock().await;

                        match peers.entry(raw_event.src_addr) {
                            Entry::Occupied(mut entry) => {
                                let peer = entry.get_mut();

                                let bytes = raw_event.bytes as u64;
                                match raw_event.direction {
                                    Direction::Ingress => peer.egress_bytes += bytes,
                                    Direction::Egress => peer.ingress_bytes += bytes,
                                }

                                peer.last_message = Some(raw_event.timestamp(boot_time));
                            }
                            Entry::Vacant(_) => {}
                        }
                    }

                    let packet = Packet {
                        src_addr: raw_event.src_addr,
                        dst_addr: raw_event.dst_addr,
                        proto: raw_event.proto,
                        bytes: raw_event.bytes,
                        timestamp: raw_event.timestamp(boot_time),
                    };

                    _ = tx.send(Event::Packet(packet));
                }
                guard.clear_ready();
            }
        }
    });

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("couldnt bind to 0.0.0.0:3000");

    let router = Router::new()
        .route("/events", get(events))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::new().allow_origin(cors::Any))
        .with_state(state);

    axum::serve(listener, router).await.unwrap();
}

fn init_ebpf(iface: &str) -> Ebpf {
    let mut ebpf = aya::Ebpf::load(aya::include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/palantir"
    )))
    .expect("failed to load ebpf program");

    match aya_log::EbpfLogger::init(&mut ebpf) {
        Err(e) => {
            warn!("failed to initialize eBPF logger: {}", e);
        }
        Ok(logger) => {
            let mut logger = AsyncFd::with_interest(logger, Interest::READABLE).unwrap();
            tokio::spawn(async move {
                loop {
                    let mut guard = logger.readable_mut().await.unwrap();
                    guard.get_inner_mut().flush();
                    guard.clear_ready();
                }
            });
        }
    }

    {
        let probe: &mut SchedClassifier = ebpf
            .program_mut("tc_ingress")
            .expect("failed to get program tc_ingress")
            .try_into()
            .unwrap();
        _ = probe.load().inspect_err(|err| warn!("{}", err));
        _ = probe
            .attach(iface, TcAttachType::Ingress)
            .inspect_err(|err| warn!("{}", err));
    }

    {
        let probe: &mut SchedClassifier = ebpf
            .program_mut("tc_egress")
            .expect("failed to get program tc_egress")
            .try_into()
            .unwrap();
        _ = probe.load().inspect_err(|err| warn!("{}", err));
        _ = probe
            .attach(iface, TcAttachType::Egress)
            .inspect_err(|err| warn!("{}", err));
    }

    ebpf
}

async fn events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<sse::Event, Infallible>>> {
    let mut rx = state.tx.subscribe();
    let peers: Vec<_> = {
        let peers = state.peers.lock().await;
        peers.values().cloned().collect()
    };

    let stream = stream! {
        for peer in peers {
            yield Ok(sse::Event::default().data(serde_json::to_string(&Event::Peer(peer)).unwrap()))
        }

        while let Ok(event) = rx.recv().await {
            yield Ok(sse::Event::default().data(serde_json::to_string(&event).unwrap()))
        }
    };

    Sse::new(stream)
}
