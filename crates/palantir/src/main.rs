#![feature(ip)]

use std::{
    collections::{HashMap, hash_map::Entry},
    convert::Infallible,
    env, fs,
    mem::zeroed,
    net::IpAddr,
    os::fd::AsRawFd,
    sync::Arc,
    time::{Duration, SystemTime},
};

use async_stream::stream;
use axum::{
    Json, Router,
    extract::State,
    response::{Sse, sse},
    routing::get,
};
use aya::{
    Ebpf,
    maps::RingBuf,
    programs::{SchedClassifier, TcAttachType},
};
use ebpf_common::event::{Direction, RawEvent};
use futures_util::Stream;
use ip_metadata::{IpMetadata, Resolver};
use libc::{CLOCK_BOOTTIME, CLOCK_REALTIME, clock_gettime, timespec};
use log::{debug, warn};

use serde::Serialize;
use tokio::{
    io::{Interest, unix::AsyncFd},
    net::TcpListener,
    sync::{Mutex, broadcast},
    time::sleep,
};
use tower_http::cors::{self, CorsLayer};

#[derive(Serialize, Clone, Debug)]
struct Config {
    iface: String,
    peer_idle_timeout: Duration,
    lat: f64,
    lon: f64,
}

struct AppState {
    tx: broadcast::Sender<Event>,
    peers: Arc<Mutex<HashMap<IpAddr, Peer>>>,
    config: Config,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "snake_case")]
enum Event {
    Peer(Peer),
    Update {
        addr: IpAddr,
        ingress_bytes: u64,
        egress_bytes: u64,
        last_packet_ts: SystemTime,
    },
    Inactive {
        addr: IpAddr,
    },
}

#[derive(Serialize, Clone)]
struct Peer {
    pub addr: IpAddr,
    pub metadata: IpMetadata,
    pub ingress_bytes: u64,
    pub egress_bytes: u64,
    pub last_packet_ts: SystemTime,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let config = Config {
        iface: env::var("IFACE").expect("IFACE is not defined"),
        peer_idle_timeout: Duration::from_secs(
            env::var("PEER_IDLE_TIMEOUT")
                .expect("PEER_IDLE_TIMEOUT is not defined")
                .parse()
                .expect("PEER_IDLE_TIMEOUT is not a valid u64"),
        ),
        lat: env::var("SERVER_LAT")
            .expect("SERVER_LAT is not defined")
            .parse()
            .expect("SERVER_LAT is not a valid f64"),
        lon: env::var("SERVER_LON")
            .expect("SERVER_LON is not defined")
            .parse()
            .expect("SERVER_LON is not a valid f64"),
    };

    debug!("{:#?}", config);

    let (tx, _) = broadcast::channel(64);

    let state = Arc::new(AppState {
        peers: Arc::new(Mutex::new(HashMap::new())),
        config,
        tx,
    });

    tokio::spawn({
        let state = state.clone();
        let mut ebpf = init_ebpf(&state.config);
        let boot_time = boot_time();
        let resolver = unsafe {
            Resolver::new(
                maxminddb::Reader::open_mmap("assets/asn.mmdb").expect("failed to open asn db"),
                maxminddb::Reader::open_mmap("assets/geo.mmdb").expect("failed to open geo db"),
                serde_json::from_slice(
                    &fs::read("assets/country-metadata.json").expect("failed to read metadat file"),
                )
                .expect("failed to parse metadata file"),
            )
        };

        async move {
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

                    let mut peers = state.peers.lock().await;

                    match peers.entry(peer_addr) {
                        Entry::Occupied(mut entry) => {
                            let peer = entry.get_mut();

                            match raw_event.direction {
                                Direction::Ingress => peer.egress_bytes += raw_event.bytes as u64,
                                Direction::Egress => peer.ingress_bytes += raw_event.bytes as u64,
                            };

                            peer.last_packet_ts = raw_event.timestamp(boot_time);

                            _ = state.tx.send(Event::Update {
                                addr: peer.addr,
                                ingress_bytes: peer.ingress_bytes,
                                egress_bytes: peer.egress_bytes,
                                last_packet_ts: peer.last_packet_ts,
                            })
                        }
                        Entry::Vacant(entry) => {
                            let peer = entry.insert(Peer {
                                addr: peer_addr,
                                metadata: resolver.lookup(peer_addr),
                                ingress_bytes: 0,
                                egress_bytes: 0,
                                last_packet_ts: raw_event.timestamp(boot_time),
                            });

                            match raw_event.direction {
                                Direction::Ingress => peer.egress_bytes += raw_event.bytes as u64,
                                Direction::Egress => peer.ingress_bytes += raw_event.bytes as u64,
                            };

                            _ = state.tx.send(Event::Peer(peer.clone()));
                        }
                    }
                }
                guard.clear_ready();
            }
        }
    });

    tokio::spawn({
        let state = state.clone();
        async move {
            loop {
                let now = SystemTime::now();

                state.peers.lock().await.retain(|_, peer| {
                    let active = now.duration_since(peer.last_packet_ts).unwrap()
                        < state.config.peer_idle_timeout;

                    if !active {
                        _ = state.tx.send(Event::Inactive { addr: peer.addr });
                    }

                    active
                });
                sleep(Duration::from_secs(1)).await;
            }
        }
    });

    let app = Router::new()
        .route("/events", get(handle_events))
        .route("/config", get(handle_config))
        .layer(CorsLayer::new().allow_origin(cors::Any))
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn handle_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<sse::Event, Infallible>>> {
    let mut rx = state.tx.subscribe();

    let peers: Vec<_> = state.peers.lock().await.values().cloned().collect();

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

async fn handle_config(State(state): State<Arc<AppState>>) -> Json<Config> {
    Json(state.config.clone())
}

fn boot_time() -> SystemTime {
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

    SystemTime::UNIX_EPOCH + (real_time - boot_time)
}

fn init_ebpf(config: &Config) -> Ebpf {
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
            .attach(&config.iface, TcAttachType::Ingress)
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
            .attach(&config.iface, TcAttachType::Egress)
            .inspect_err(|err| warn!("{}", err));
    }

    ebpf
}
