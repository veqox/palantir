use core::net::IpAddr;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RawEvent {
    pub pid: u32,
    pub src_addr: IpAddr,
    pub dst_addr: IpAddr,
    pub ts_offset_ns: u64,
    pub proto: u8,
    pub direction: Direction,
    pub bytes: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn local_addr(&self) -> IpAddr {
        match self.direction {
            Direction::Ingress => self.dst_addr,
            Direction::Egress => self.src_addr,
        }
    }

    #[cfg(feature = "std")]
    pub fn timestamp(&self, boot_time: SystemTime) -> SystemTime {
        boot_time + Duration::from_nanos(self.ts_offset_ns)
    }
}
