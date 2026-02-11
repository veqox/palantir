#![no_std]
#![no_main]

use core::{
    ffi::c_long,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use aya_ebpf::{
    bindings::{BPF_RB_FORCE_WAKEUP, TC_ACT_OK},
    helpers::{bpf_get_current_pid_tgid, generated::bpf_ktime_get_ns},
    macros::{classifier, map},
    maps::RingBuf,
    programs::TcContext,
};
use aya_log_ebpf::warn;
use ebpf_common::{
    eth::{ETH_TYPE_IPV4, ETH_TYPE_IPV6, EthHdr},
    event::{Direction, RawEvent},
    ip::{
        IP_PROTO_HOP_OPT, IP_PROTO_IPV6_FRAG, IP_PROTO_IPV6_OPTS, IP_PROTO_IPV6_ROUTE,
        IPV6_MAX_EXTENSION_HEADER_COUNT, Ipv4Hdr, Ipv6Hdr,
    },
};

#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(4096 * 20, 0);

#[classifier]
pub fn tc_ingress(ctx: TcContext) -> i32 {
    match try_handle_packet(&ctx, Direction::Ingress) {
        _ => TC_ACT_OK,
    }
}

#[classifier]
pub fn tc_egress(ctx: TcContext) -> i32 {
    match try_handle_packet(&ctx, Direction::Egress) {
        _ => TC_ACT_OK,
    }
}

fn try_handle_packet(ctx: &TcContext, direction: Direction) -> Result<(), c_long> {
    let pid = bpf_get_current_pid_tgid() as u32;
    let ts_offset_ns = unsafe { bpf_ktime_get_ns() };

    let eth_hdr = ctx.load::<EthHdr>(0)?;
    let eth_type = u16::from_be(eth_hdr.eth_type);

    let event = match eth_type {
        ETH_TYPE_IPV4 => {
            let ip_hdr = ctx.load::<Ipv4Hdr>(size_of::<EthHdr>())?;
            let src_addr = IpAddr::V4(Ipv4Addr::from_octets(ip_hdr.src_addr));
            let dst_addr = IpAddr::V4(Ipv4Addr::from_octets(ip_hdr.dst_addr));

            let proto = ip_hdr.proto;
            let bytes = u16::from_be_bytes(ip_hdr.tot_len);

            RawEvent {
                pid,
                src_addr,
                dst_addr,
                ts_offset_ns,
                proto,
                direction,
                bytes,
            }
        }
        ETH_TYPE_IPV6 => {
            let ip_hdr = ctx.load::<Ipv6Hdr>(size_of::<EthHdr>())?;
            let src_addr = IpAddr::V6(Ipv6Addr::from_octets(ip_hdr.src_addr));
            let dst_addr = IpAddr::V6(Ipv6Addr::from_octets(ip_hdr.dst_addr));

            let mut offset = size_of::<EthHdr>() + size_of::<Ipv6Hdr>();
            let mut next_header = ip_hdr.next_hdr;

            for _ in 0..IPV6_MAX_EXTENSION_HEADER_COUNT {
                if offset + 1 >= ctx.data_end() - ctx.data() {
                    return Ok(());
                }

                match next_header {
                    IP_PROTO_HOP_OPT | IP_PROTO_IPV6_ROUTE | IP_PROTO_IPV6_OPTS
                    | IP_PROTO_IPV6_FRAG => {
                        let extension_header_length = ctx.load::<u8>(offset + 1)?;
                        offset += (extension_header_length as usize + 1) * 8;
                    }
                    _ => {
                        break;
                    }
                }

                next_header = ctx.load::<u8>(offset)?;
            }

            let proto = next_header;

            let bytes = u16::from_be_bytes(ip_hdr.payload_len);

            RawEvent {
                pid,
                src_addr,
                dst_addr,
                ts_offset_ns,
                proto,
                direction,
                bytes,
            }
        }
        _ => {
            return Ok(());
        }
    };

    match EVENTS.reserve::<RawEvent>(0) {
        Some(mut entry) => {
            entry.write(event);
            entry.submit(BPF_RB_FORCE_WAKEUP.into());
        }
        None => {
            warn!(ctx, "EVENTS is full: skipping");
        }
    };

    Ok(())
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
