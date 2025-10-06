#![no_std]

use core::net::IpAddr;

pub const EVENT_TYPE_OPEN: u8 = 0;
pub const EVENT_TYPE_CLOSE: u8 = 1;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Event {
    pub r#type: u8,
    pub pid: u32,
    pub ts_offset_ns: u64,
    pub src_addr: IpAddr,
    pub dst_addr: IpAddr,
    pub src_port: u16,
    pub dst_port: u16,
}
