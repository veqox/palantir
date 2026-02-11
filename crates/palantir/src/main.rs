#![feature(ip)]

use std::{
    collections::HashMap,
    env,
    mem::zeroed,
    net::IpAddr,
    os::fd::AsRawFd,
    time::{Duration, SystemTime},
};

use aya::{
    maps::RingBuf,
    programs::{SchedClassifier, TcAttachType},
};
use ebpf_common::event::{Direction, RawEvent};
use libc::{CLOCK_BOOTTIME, CLOCK_REALTIME, clock_gettime, timespec};
use log::warn;
use maxminddb::{
    Reader,
    geoip2::{self},
};
use tokio::{
    io::{Interest, unix::AsyncFd},
    main,
};

#[main]
async fn main() {
    env_logger::init();

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

    let iface = env::var("IFACE").expect("IFACE is not defined");

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
            .attach(&iface, TcAttachType::Ingress)
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
            .attach(&iface, TcAttachType::Egress)
            .inspect_err(|err| warn!("{}", err));
    }

    let reader = unsafe { Reader::open_mmap("assets/GeoLite2-City.mmdb") }.unwrap();
    let mut events = RingBuf::try_from(ebpf.map_mut("EVENTS").unwrap()).unwrap();
    let mut peers: HashMap<IpAddr, Peer> = HashMap::new();

    let poll = AsyncFd::new(events.as_raw_fd()).unwrap();
    loop {
        let mut guard = poll.readable().await.unwrap();
        while let Some(item) = events.next() {
            let raw_event = unsafe { *(item.as_ptr() as *const RawEvent) };
            let peer_addr = raw_event.peer_addr();

            if peer_addr.is_multicast() || !peer_addr.is_global() {
                continue;
            }

            let peer = peers.entry(peer_addr).or_insert_with(|| Peer {
                addr: peer_addr,
                location: resolve_location(&reader, peer_addr),
                ingress_bytes: 0,
                egress_bytes: 0,
                last_packet_ts: raw_event.timestamp(boot_time),
            });

            match raw_event.direction {
                Direction::Ingress => peer.egress_bytes += raw_event.bytes as u64,
                Direction::Egress => peer.ingress_bytes += raw_event.bytes as u64,
            };
        }
        guard.clear_ready();
    }
}

fn resolve_location<R>(reader: &Reader<R>, addr: IpAddr) -> Location
where
    R: AsRef<[u8]>,
{
    let record = match reader.lookup(addr) {
        Ok(r) if r.has_data() => match r.decode::<geoip2::City>() {
            Ok(Some(r)) => r,
            _ => return Location::Unknown,
        },
        _ => return Location::Unknown,
    };

    if let (Some(lat), Some(lon), Some(accuracy_radius), Some(country_iso_code), Some(city_name)) = (
        record.location.latitude,
        record.location.longitude,
        record.location.accuracy_radius,
        record.country.iso_code,
        record.city.names.english,
    ) {
        return Location::City {
            lat,
            lon,
            country_iso_code: country_iso_code.to_string(),
            city_name: city_name.to_string(),
            accuracy_radius,
        };
    }

    if let Some(country_iso_code) = record.registered_country.iso_code {
        return Location::RegisteredCountry {
            country_iso_code: country_iso_code.to_string(),
        };
    }

    Location::Unknown
}

#[derive(Clone, Debug)]
#[allow(unused)]
struct Peer {
    pub addr: IpAddr,
    pub location: Location,
    pub ingress_bytes: u64,
    pub egress_bytes: u64,
    pub last_packet_ts: SystemTime,
}

#[derive(Clone, Debug)]
#[allow(unused)]
enum Location {
    City {
        lat: f64,
        lon: f64,
        city_name: String,
        country_iso_code: String,
        accuracy_radius: u16,
    },
    RegisteredCountry {
        country_iso_code: String,
    },
    Unknown,
}
