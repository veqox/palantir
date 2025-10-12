use std::net::IpAddr;

use net::ip::IpProto;
use palantir_ebpf_common::{DIRECTION_EGRESS, DIRECTION_INGRESS, RawEvent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize)]
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
    pub src_location: Option<LocationData>,
    pub dst_location: Option<LocationData>,
}

impl TryFrom<RawEvent> for Event {
    type Error = ();

    fn try_from(value: RawEvent) -> Result<Self, Self::Error> {
        let direction = value.direction.try_into()?;

        Ok(Event {
            pid: value.pid,
            src_addr: value.src_addr,
            dst_addr: value.dst_addr,
            src_port: value.src_port,
            dst_port: value.dst_port,
            ts_offset_ns: value.ts_offset_ns,
            proto: value.proto,
            fragment: value.fragment,
            last_fragment: value.last_fragment,
            direction,
            bytes: value.bytes,
            src_location: None,
            dst_location: None,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum Direction {
    Ingress,
    Egress,
}

impl TryFrom<u8> for Direction {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            DIRECTION_INGRESS => Ok(Self::Ingress),
            DIRECTION_EGRESS => Ok(Self::Egress),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LocationData {
    pub lon: f32,
    pub lat: f32,
}
