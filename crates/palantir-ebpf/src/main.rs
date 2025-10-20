#![no_std]
#![no_main]

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use aya_ebpf::{
    bindings::{BPF_RB_FORCE_WAKEUP, TC_ACT_OK},
    helpers::{bpf_get_current_pid_tgid, generated::bpf_ktime_get_ns},
    macros::{classifier, map},
    maps::RingBuf,
    programs::TcContext,
};

use aya_log_ebpf::warn;
use net::{
    eth::{ETHER_TYPE_IPV4, ETHER_TYPE_IPV6, EthHdr},
    ip::{IpProto, Ipv4Hdr, Ipv6Hdr},
    tcp::TcpHdr,
    udp::UdpHdr,
};
use palantir_ebpf_common::{Direction, RawEvent};

const IPV6_MAX_EXTENSION_HEADEAR_COUNT: usize = 8;

#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(4096 * 20, 0);

#[classifier]
pub fn tc_ingress(ctx: TcContext) -> i32 {
    match try_handle_packet(&ctx, Direction::Ingress) {
        Ok(ret) => ret,
        Err(_) => TC_ACT_OK,
    }
}

#[classifier]
pub fn tc_egress(ctx: TcContext) -> i32 {
    match try_handle_packet(&ctx, Direction::Egress) {
        Ok(ret) => ret,
        Err(_) => TC_ACT_OK,
    }
}

fn try_handle_packet(ctx: &TcContext, direction: Direction) -> Result<i32, ()> {
    let pid = bpf_get_current_pid_tgid() as u32;
    let ts_offset_ns = unsafe { bpf_ktime_get_ns() };

    let eth_header = ctx.load::<EthHdr>(0).or(Err(()))?;

    let event = match u16::from_be(eth_header.ether_type) {
        ETHER_TYPE_IPV4 => {
            let ip_header = ctx.load::<Ipv4Hdr>(size_of::<EthHdr>()).or(Err(()))?;
            let src_addr = IpAddr::V4(Ipv4Addr::from_bits(u32::from_be_bytes(ip_header.src_addr)));
            let dst_addr = IpAddr::V4(Ipv4Addr::from_bits(u32::from_be_bytes(ip_header.dst_addr)));
            let proto = ip_header.proto;
            let (src_port, dst_port) = match proto {
                IpProto::Tcp => {
                    let tcp_header = ctx
                        .load::<TcpHdr>(size_of::<EthHdr>() + size_of::<Ipv4Hdr>())
                        .or(Err(()))?;
                    (
                        u16::from_be_bytes(tcp_header.source),
                        u16::from_be_bytes(tcp_header.dest),
                    )
                }
                IpProto::Udp => {
                    let udp_header = ctx
                        .load::<UdpHdr>(size_of::<EthHdr>() + size_of::<Ipv4Hdr>())
                        .or(Err(()))?;
                    (
                        u16::from_be_bytes(udp_header.src),
                        u16::from_be_bytes(udp_header.dst),
                    )
                }
                _ => return Err(()),
            };
            let frags = u16::from_be_bytes(ip_header.frags);
            let frag_flags = (frags >> 13) as u8;
            let frag_offset = frags & 0x1FFF;

            let fragment = (frag_flags & 0b001) != 0 && frag_offset == 0;
            let last_fragment = (frag_flags & 0b001) == 0 && frag_offset == 0;
            let bytes = u16::from_be_bytes(ip_header.tot_len);

            RawEvent {
                pid,
                ts_offset_ns,
                src_addr,
                dst_addr,
                proto,
                src_port,
                dst_port,
                fragment,
                last_fragment,
                direction,
                bytes,
            }
        }
        ETHER_TYPE_IPV6 => {
            let ip_header = ctx.load::<Ipv6Hdr>(size_of::<EthHdr>()).or(Err(()))?;

            let src_addr = IpAddr::V6(Ipv6Addr::from_octets(ip_header.src_addr));
            let dst_addr = IpAddr::V6(Ipv6Addr::from_octets(ip_header.dst_addr));
            let mut fragment = false;
            let mut last_fragment = false;

            let mut offset = size_of::<EthHdr>() + size_of::<Ipv6Hdr>();
            let mut next_header = ip_header.next_hdr;

            for _ in 0..IPV6_MAX_EXTENSION_HEADEAR_COUNT {
                if offset + 1 >= ctx.data_end() - ctx.data() {
                    return Err(());
                }

                match next_header {
                    IpProto::HopOpt | IpProto::Ipv6Route | IpProto::Ipv6Opts => {
                        let extension_header_length = ctx.load::<u8>(offset + 1).or(Err(()))?;
                        offset += (extension_header_length as usize + 1) * 8;
                    }
                    IpProto::Ipv6Frag => {
                        fragment = true;

                        let frag_field = u16::from_be(ctx.load::<u16>(offset + 2).or(Err(()))?);
                        let frag_offset = frag_field >> 3;

                        last_fragment = (frag_field & 0b001) != 0 && frag_offset != 0;

                        const IPV6_FRAG_HDR_LEN: usize = 8;

                        offset += IPV6_FRAG_HDR_LEN;
                    }
                    IpProto::Tcp | IpProto::Udp => {
                        break;
                    }
                    _ => return Err(()),
                }

                next_header = ctx.load::<IpProto>(offset).or(Err(()))?;
            }

            let proto = next_header;

            let (src_port, dst_port) = match proto {
                IpProto::Tcp => {
                    let tcp_header = ctx
                        .load::<TcpHdr>(size_of::<EthHdr>() + size_of::<Ipv6Addr>())
                        .or(Err(()))?;
                    (
                        u16::from_be_bytes(tcp_header.source),
                        u16::from_be_bytes(tcp_header.dest),
                    )
                }
                IpProto::Udp => {
                    let udp_header = ctx
                        .load::<UdpHdr>(size_of::<EthHdr>() + size_of::<Ipv6Addr>())
                        .or(Err(()))?;
                    (
                        u16::from_be_bytes(udp_header.src),
                        u16::from_be_bytes(udp_header.dst),
                    )
                }
                _ => return Err(()),
            };
            let bytes = u16::from_be_bytes(ip_header.payload_len);

            RawEvent {
                pid,
                ts_offset_ns,
                src_addr,
                dst_addr,
                proto,
                src_port,
                dst_port,
                fragment,
                last_fragment,
                direction,
                bytes,
            }
        }
        _ => {
            return Err(());
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

    Ok(TC_ACT_OK)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
