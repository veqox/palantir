use std::{net::IpAddr, time::SystemTime};

use net::ip::IpProto;
use serde::Serialize;

use crate::api::IpInfo;

#[derive(Debug, Clone, Serialize)]
pub enum Event {
    #[serde(rename = "peer")]
    Peer(Peer),
    #[serde(rename = "packet")]
    Packet(Packet),
}

#[derive(Debug, Clone, Serialize)]
pub struct Peer {
    pub addr: IpAddr,
    pub info: IpInfo,
    pub ingress_bytes: u64,
    pub egress_bytes: u64,
    pub last_message: Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Packet {
    pub proto: IpProto,
    pub src_addr: IpAddr,
    pub dst_addr: IpAddr,
    pub bytes: u16,
    pub timestamp: SystemTime,
}
