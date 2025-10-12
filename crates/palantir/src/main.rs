#![feature(ip)]

mod event;

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
use palantir_ebpf_common::{self, RawEvent};
use reqwest::Client;
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

use crate::event::{Direction, Event, LocationData};

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<Event>,
}

#[tokio::main]
async fn main() {
    let location = LocationData {
        lat: env::var("LOCATION_LAT")
            .expect("LOCATION_LAT is not defined")
            .parse()
            .expect("LOCATION_LAT is not a valid f32"),
        lon: env::var("LOCATION_LON")
            .expect("LOCATION_LON is not defined")
            .parse()
            .expect("LOCATION_LON is not a valid f32"),
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

    let state = Arc::new(AppState { tx: tx.clone() });

    tokio::spawn({
        let tx = tx.clone();
        async move {
            let client = Client::new();
            let mut cache = HashMap::<IpAddr, LocationData>::new();

            let mut ebpf = init_ebpf(&iface);

            let mut events = RingBuf::try_from(ebpf.map_mut("EVENTS").unwrap()).unwrap();

            let poll = AsyncFd::new(events.as_raw_fd()).unwrap();
            loop {
                let mut guard = poll.readable().await.unwrap();
                while let Some(item) = events.next() {
                    let raw_event = unsafe { *(item.as_ptr() as *const RawEvent) };

                    if raw_event.src_addr.is_multicast()
                        || raw_event.dst_addr.is_multicast()
                        || (!raw_event.src_addr.is_global() && !raw_event.dst_addr.is_global())
                    {
                        continue;
                    }

                    trace!("{:?}", raw_event);

                    if raw_event.src_addr.is_global()
                        && let Entry::Vacant(entry) = cache.entry(raw_event.src_addr)
                    {
                        match client
                            .get(format!("http://ip-api.com/json/{}", raw_event.src_addr))
                            .send()
                            .await
                            .unwrap()
                            .json()
                            .await
                        {
                            Ok(info) => {
                                _ = entry.insert(info);
                            }
                            Err(_) => {
                                warn!("failed to get ip info for {}, skipping", raw_event.src_addr);
                                continue;
                            }
                        };
                    }

                    if raw_event.dst_addr.is_global()
                        && let Entry::Vacant(entry) = cache.entry(raw_event.dst_addr)
                    {
                        match client
                            .get(format!("http://ip-api.com/json/{}", raw_event.dst_addr))
                            .send()
                            .await
                            .unwrap()
                            .json()
                            .await
                        {
                            Ok(info) => {
                                _ = entry.insert(info);
                            }
                            Err(_) => {
                                warn!("failed to get ip info for {}, skipping", raw_event.dst_addr);
                                continue;
                            }
                        };
                    }

                    let mut event = match Event::try_from_raw(raw_event, boot_time) {
                        Ok(event) => event,
                        Err(_) => {
                            warn!("failed to convert RawEvent to Event");
                            continue;
                        }
                    };

                    match event.direction {
                        Direction::Ingress => {
                            event.src_location = cache.get(&event.src_addr).cloned();
                            event.dst_location = Some(location);
                        }
                        Direction::Egress => {
                            event.src_location = Some(location);
                            event.dst_location = cache.get(&event.dst_addr).cloned();
                        }
                    }

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

    let stream = stream! {
        while let Ok(event) = rx.recv().await {
            yield Ok(sse::Event::default().data(serde_json::to_string(&event).unwrap()))
        }
    };

    Sse::new(stream)
}
