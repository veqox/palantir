#![no_std]

use core::net::IpAddr;
use net::ip::IpProto;

pub const DIRECTION_INGRESS: u8 = 0;
pub const DIRECTION_EGRESS: u8 = 1;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RawEvent {
    pub pid: u32,
    pub src_addr: IpAddr,
    pub dst_addr: IpAddr,
    pub src_port: u16,
    pub dst_port: u16,
    pub ts_offset_ns: u64,
    pub proto: IpProto,
    pub fragment: bool,
    pub last_fragment: bool,
    pub direction: u8,
    pub bytes: u16,
}
