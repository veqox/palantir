#![no_std]

use core::net::IpAddr;
use net::ip::IpProto;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
use std::time::SystemTime;

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
    pub direction: Direction,
    pub bytes: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub enum Direction {
    Ingress,
    Egress,
}

impl RawEvent {
    pub fn peer_addr(&self) -> IpAddr {
        match self.direction {
            Direction::Ingress => self.src_addr,
            Direction::Egress => self.dst_addr,
        }
    }

    #[cfg(feature = "std")]
    pub fn timestamp(&self, boot_time: SystemTime) -> SystemTime {
        use core::time::Duration;

        boot_time + Duration::from_nanos(self.ts_offset_ns)
    }
}
