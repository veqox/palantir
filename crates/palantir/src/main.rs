use std::os::fd::AsRawFd;

use aya::{maps::RingBuf, programs::KProbe};
use log::{info, warn};
use models::Event;
use tokio::io::{Interest, unix::AsyncFd};

#[tokio::main]
async fn main() {
    env_logger::init();

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
            info!("{:?}", unsafe { *(item.as_ptr() as *const Event) });
        }
        guard.clear_ready();
    }
}
