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
use aya::{maps::RingBuf, programs::KProbe};
use futures_util::Stream;
use palantir_ebpf_common;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{Interest, unix::AsyncFd},
    net::TcpListener,
    sync::broadcast,
};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Serialize, Clone)]
struct Event {
    pub r#type: u8,
    pub pid: u32,
    pub src_addr: IpAddr,
    pub dst_addr: IpAddr,
    pub src_port: u16,
    pub dst_port: u16,
    pub src_location: Option<IpInfo>,
    pub dst_location: Option<IpInfo>,
}

#[derive(Serialize, Deserialize, Clone)]
struct IpInfo {
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
            let src_location = Some(IpInfo {
                lat: env::var("SRC_LOCATION_LAT")
                    .expect("SRC_LOCATION_LAT is not defined")
                    .parse()
                    .expect("SRC_LOCATION_LAT is not a valid f32"),
                lon: env::var("SRC_LOCATION_LON")
                    .expect("SRC_LOCATION_LON is not defined")
                    .parse()
                    .expect("SRC_LOCATION_LON is not a valid f32"),
            });

            let client = Client::new();
            let mut cache = HashMap::<IpAddr, IpInfo>::new();

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

            for (name, program) in ebpf.programs_mut() {
                let probe: &mut KProbe = program.try_into().unwrap();
                _ = probe.load();
                _ = probe.attach(name, 0);
                info!("attached program {}", name);
            }

            let mut events = RingBuf::try_from(ebpf.map_mut("EVENTS").unwrap()).unwrap();

            let poll = AsyncFd::new(events.as_raw_fd()).unwrap();
            loop {
                let mut guard = poll.readable().await.unwrap();
                while let Some(item) = events.next() {
                    let event = unsafe { *(item.as_ptr() as *const palantir_ebpf_common::Event) };
                    info!("{:?}", event);

                    if event.src_addr.is_multicast()
                        || event.dst_addr.is_multicast()
                        || (!event.src_addr.is_global() && !event.dst_addr.is_global())
                    {
                        continue;
                    }

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

                    let event = Event {
                        r#type: event.r#type,
                        pid: event.pid,
                        src_addr: event.src_addr,
                        dst_addr: event.dst_addr,
                        src_port: event.src_port,
                        dst_port: event.dst_port,
                        src_location: src_location.clone(),
                        dst_location: cache.get(&event.dst_addr).cloned(),
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
