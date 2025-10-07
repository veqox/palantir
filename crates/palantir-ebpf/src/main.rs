#![no_std]
#![no_main]

mod sock;

use palantir_ebpf_common::{EVENT_TYPE_CLOSE, EVENT_TYPE_OPEN, Event};
use sock::{AF_INET, AF_INET6, Sock};

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use aya_ebpf::{
    bindings::BPF_RB_FORCE_WAKEUP,
    helpers::{bpf_get_current_pid_tgid, bpf_probe_read_kernel},
    macros::{kprobe, map},
    maps::RingBuf,
    programs::ProbeContext,
};
use aya_log_ebpf::{trace, warn};

#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(2 << 12, 0);

#[kprobe]
pub fn tcp_connect(ctx: ProbeContext) -> u32 {
    match try_handle_event(&ctx, EVENT_TYPE_OPEN) {
        Ok(ret) => ret,
        Err(ret) => {
            warn!(&ctx, "probe tcp_connect failed with {}", ret);
            ret.try_into().unwrap_or(1)
        }
    }
}

#[kprobe]
pub fn tcp_accept(ctx: ProbeContext) -> u32 {
    match try_handle_event(&ctx, EVENT_TYPE_OPEN) {
        Ok(ret) => ret,
        Err(ret) => {
            warn!(&ctx, "probe tcp_accept failed with {}", ret);
            ret.try_into().unwrap_or(1)
        }
    }
}

#[kprobe]
pub fn tcp_close(ctx: ProbeContext) -> u32 {
    match try_handle_event(&ctx, EVENT_TYPE_CLOSE) {
        Ok(ret) => ret,
        Err(ret) => {
            warn!(&ctx, "probe tcp_close failed with {}", ret);
            ret.try_into().unwrap_or(1)
        }
    }
}

fn try_handle_event(ctx: &ProbeContext, r#type: u8) -> Result<u32, i64> {
    let sock: *mut Sock = ctx.arg(0).ok_or(1)?;

    let pid = bpf_get_current_pid_tgid() as u32;

    let skc_family = unsafe { bpf_probe_read_kernel(&(*sock).sock_common.family) }?;
    let dst_port = u16::from_be(unsafe { bpf_probe_read_kernel(&(*sock).sock_common.dport)? });
    let src_port = unsafe { bpf_probe_read_kernel(&(*sock).sock_common.num)? };

    let event = match skc_family {
        AF_INET => {
            let dst_addr = IpAddr::V4(Ipv4Addr::from_bits(u32::from_be(unsafe {
                bpf_probe_read_kernel(&(*sock).sock_common.daddr)
            }?)));
            let src_addr = IpAddr::V4(Ipv4Addr::from_bits(u32::from_be(unsafe {
                bpf_probe_read_kernel(&(*sock).sock_common.rcv_saddr)
            }?)));

            Some(Event {
                r#type,
                dst_addr,
                dst_port,
                src_addr,
                src_port,
                pid,
            })
        }
        AF_INET6 => {
            let dst_addr = IpAddr::V6(Ipv6Addr::from_octets(unsafe {
                bpf_probe_read_kernel(&(*sock).sock_common.v6_daddr)
            }?));
            let src_addr = IpAddr::V6(Ipv6Addr::from_octets(unsafe {
                bpf_probe_read_kernel(&(*sock).sock_common.v6_rcv_saddr)
            }?));

            Some(Event {
                r#type,
                dst_addr,
                dst_port,
                src_addr,
                src_port,
                pid,
            })
        }
        skc_family => {
            warn!(ctx, "Unkown sock_family {}", skc_family);
            None
        }
    };

    match event {
        Some(event) => {
            trace!(
                ctx,
                "type: {}, src_addr: {}, src_port: {} dst_addr: {}, dst_port: {}, pid: {}",
                if event.r#type == 0 { "OPEN" } else { "CLOSE" },
                event.src_addr,
                event.src_port,
                event.dst_addr,
                event.dst_port,
                event.pid,
            );

            match EVENTS.reserve::<Event>(0) {
                Some(mut entry) => {
                    entry.write(event);
                    entry.submit(BPF_RB_FORCE_WAKEUP.into());
                    Ok(0)
                }
                None => {
                    warn!(ctx, "EVENTS is full: skipping");
                    Err(1)
                }
            }
        }
        None => Err(1),
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
