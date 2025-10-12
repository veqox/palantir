#![no_std]

use core::net::IpAddr;
use net::ip::IpProto;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Event {
    pub pid: u32,
    pub src_addr: IpAddr,
    pub dst_addr: IpAddr,
    pub src_port: u16,
    pub dst_port: u16,
    pub ts_offset_ns: u64,
    pub proto: IpProto,
    pub fragment: bool,
    pub last_fragment: bool,
    pub direction: Direction,
    pub bytes: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub enum Direction {
    Ingress,
    Egress,
}
