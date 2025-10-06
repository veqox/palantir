#![no_std]
#![no_main]

mod sock;

use sock::{AF_INET, AF_INET6, Sock};

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use aya_ebpf::{helpers::bpf_probe_read_kernel, macros::kprobe, programs::ProbeContext};
use aya_log_ebpf::{info, warn};

#[kprobe]
pub fn palantir(ctx: ProbeContext) -> u32 {
    match try_palantir(&ctx) {
        Ok(ret) => ret,
        Err(ret) => {
            warn!(&ctx, "failed with {}", ret);
            ret.try_into().unwrap_or(1)
        }
    }
}

fn try_palantir(ctx: &ProbeContext) -> Result<u32, i64> {
    let sock: *mut Sock = ctx.arg(0).ok_or(1)?;

    let skc_family = unsafe { bpf_probe_read_kernel(&(*sock).sock_common.family) }?;

    match skc_family {
        AF_INET => {
            let dst_addr = IpAddr::V4(Ipv4Addr::from_bits(u32::from_be(unsafe {
                bpf_probe_read_kernel(&(*sock).sock_common.daddr)
            }?)));
            let src_addr = IpAddr::V4(Ipv4Addr::from_bits(u32::from_be(unsafe {
                bpf_probe_read_kernel(&(*sock).sock_common.rcv_saddr)
            }?)));

            info!(ctx, "AF_INET src: {}, dst: {}", src_addr, dst_addr);
        }
        AF_INET6 => {
            let dst_addr = IpAddr::V6(Ipv6Addr::from_octets(unsafe {
                bpf_probe_read_kernel(&(*sock).sock_common.v6_daddr)
            }?));
            let src_addr = IpAddr::V6(Ipv6Addr::from_octets(unsafe {
                bpf_probe_read_kernel(&(*sock).sock_common.v6_rcv_saddr)
            }?));

            info!(ctx, "AF_INET6 src: {}, dst: {}", src_addr, dst_addr);
        }
        skc_family => {
            warn!(ctx, "Unkown sock_family {}", skc_family);
        }
    }

    Ok(0)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
