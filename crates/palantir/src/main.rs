use aya::programs::KProbe;
use log::{info, warn};
use tokio::{
    io::{Interest, unix::AsyncFd},
    signal,
};

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

        info!("attached program {}", name);
        _ = probe.attach(name, 0);
    }

    let ctrl_c = signal::ctrl_c();
    println!("Waiting for Ctrl-C...");
    ctrl_c.await.unwrap();
    println!("Exiting...");
}
