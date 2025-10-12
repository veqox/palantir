#![feature(ip)]

use std::{
    collections::HashMap, convert::Infallible, env, net::IpAddr, os::fd::AsRawFd, sync::Arc,
};

use async_stream::stream;
use axum::{
    Router,
    extract::State,
    response::{Sse, sse},
    routing::get,
};
use aya::{
    maps::RingBuf,
    programs::{SchedClassifier, TcAttachType},
};
use futures_util::Stream;
use net::ip::IpProto;
use palantir_ebpf_common;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{Interest, unix::AsyncFd},
    net::TcpListener,
    sync::broadcast,
};
use tower_http::{
    cors::{self, CorsLayer},
    trace::TraceLayer,
};
use tracing::{trace, warn};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Copy, Serialize)]
struct Event {
    pub pid: u32,
    pub src_addr: IpAddr,
    pub dst_addr: IpAddr,
    pub src_port: u16,
    pub dst_port: u16,
    pub ts_offset_ns: u64,
    pub proto: IpProto,
    pub fragment: bool,
    pub last_fragment: bool,
    pub direction: Direction,
    pub bytes: u16,
    pub src_location: Option<LocationData>,
    pub dst_location: Option<LocationData>,
}

#[derive(Debug, Clone, Copy, Serialize)]
enum Direction {
    Ingress,
    Egress,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct LocationData {
    pub lon: f32,
    pub lat: f32,
}

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<Event>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let (tx, _) = broadcast::channel(64);

    let state = Arc::new(AppState { tx: tx.clone() });

    tokio::spawn({
        let tx = tx.clone();
        async move {
            let location = Some(LocationData {
                lat: env::var("LOCATION_LAT")
                    .expect("LOCATION_LAT is not defined")
                    .parse()
                    .expect("LOCATION_LAT is not a valid f32"),
                lon: env::var("LOCATION_LON")
                    .expect("LOCATION_LON is not defined")
                    .parse()
                    .expect("LOCATION_LON is not a valid f32"),
            });

            let client = Client::new();
            let mut cache = HashMap::<IpAddr, LocationData>::new();

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
                    .attach("enp31s0", TcAttachType::Ingress)
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
                    .attach("enp31s0", TcAttachType::Egress)
                    .inspect_err(|err| warn!("{}", err));
            }

            let mut events = RingBuf::try_from(ebpf.map_mut("EVENTS").unwrap()).unwrap();

            let poll = AsyncFd::new(events.as_raw_fd()).unwrap();
            loop {
                let mut guard = poll.readable().await.unwrap();
                while let Some(item) = events.next() {
                    let event = unsafe { *(item.as_ptr() as *const palantir_ebpf_common::Event) };

                    if event.src_addr.is_multicast()
                        || event.dst_addr.is_multicast()
                        || (!event.src_addr.is_global() && !event.dst_addr.is_global())
                    {
                        continue;
                    }

                    trace!("{:?}", event);

                    if event.src_addr.is_global() {
                        if !cache.contains_key(&event.src_addr) {
                            match client
                                .get(format!("http://ip-api.com/json/{}", event.src_addr))
                                .send()
                                .await
                                .unwrap()
                                .json()
                                .await
                            {
                                Ok(info) => {
                                    _ = cache.insert(event.src_addr, info);
                                }
                                Err(_) => {
                                    warn!("failed to get ip info for {}, skipping", event.src_addr);
                                    continue;
                                }
                            };
                        }
                    }

                    if event.dst_addr.is_global() {
                        if !cache.contains_key(&event.dst_addr) {
                            match client
                                .get(format!("http://ip-api.com/json/{}", event.dst_addr))
                                .send()
                                .await
                                .unwrap()
                                .json()
                                .await
                            {
                                Ok(info) => {
                                    _ = cache.insert(event.dst_addr, info);
                                }
                                Err(_) => {
                                    warn!("failed to get ip info for {}, skipping", event.dst_addr);
                                    continue;
                                }
                            };
                        }
                    }

                    let event = Event {
                        pid: event.pid,
                        src_addr: event.src_addr,
                        dst_addr: event.dst_addr,
                        src_port: event.src_port,
                        dst_port: event.dst_port,
                        ts_offset_ns: event.ts_offset_ns,
                        proto: event.proto,
                        fragment: event.fragment,
                        last_fragment: event.last_fragment,
                        direction: match event.direction {
                            palantir_ebpf_common::Direction::Ingress => Direction::Ingress,
                            palantir_ebpf_common::Direction::Egress => Direction::Egress,
                        },
                        bytes: event.bytes,
                        src_location: cache.get(&event.src_addr).cloned().or_else(|| {
                            match event.direction {
                                palantir_ebpf_common::Direction::Egress => location,
                                palantir_ebpf_common::Direction::Ingress => None,
                            }
                        }),
                        dst_location: cache.get(&event.dst_addr).cloned().or_else(|| {
                            match event.direction {
                                palantir_ebpf_common::Direction::Egress => None,
                                palantir_ebpf_common::Direction::Ingress => location,
                            }
                        }),
                    };

                    _ = tx.send(event);
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

async fn events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<sse::Event, Infallible>>> {
    let mut rx = state.tx.subscribe();

    let stream = stream! {
        while let Ok(event) = rx.recv().await {
            yield Ok(sse::Event::default().data(serde_json::to_string(&event).unwrap()))
        }
    };

    Sse::new(stream)
}
