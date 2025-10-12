#[repr(C, packed)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[derive(Debug, Copy, Clone)]
pub struct Ipv4Hdr {
    pub vihl: u8,
    pub tos: u8,
    pub tot_len: [u8; 2],
    pub id: [u8; 2],
    pub frags: [u8; 2],
    pub ttl: u8,
    pub proto: IpProto,
    pub check: [u8; 2],
    pub src_addr: [u8; 4],
    pub dst_addr: [u8; 4],
}

#[repr(C, packed)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[derive(Debug, Copy, Clone)]
pub struct Ipv6Hdr {
    pub vcf: [u8; 4],
    pub payload_len: [u8; 2],
    pub next_hdr: IpProto,
    pub hop_limit: u8,
    pub src_addr: [u8; 16],
    pub dst_addr: [u8; 16],
}

#[repr(u8)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[derive(Debug, Copy, Clone)]
pub enum IpProto {
    HopOpt = 0,
    Ipv4 = 4,
    Tcp = 6,
    Udp = 17,
    Ipv6 = 41,
    Ipv6Route = 43,
    Ipv6Frag = 44,
    Ipv6Opts = 60,
}
