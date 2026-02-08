#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct Ipv4Hdr {
    pub vihl: u8,
    pub tos: u8,
    pub tot_len: [u8; 2],
    pub id: [u8; 2],
    pub frags: [u8; 2],
    pub ttl: u8,
    pub proto: u8,
    pub check: [u8; 2],
    pub src_addr: [u8; 4],
    pub dst_addr: [u8; 4],
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct Ipv6Hdr {
    pub vcf: [u8; 4],
    pub payload_len: [u8; 2],
    pub next_hdr: u8,
    pub hop_limit: u8,
    pub src_addr: [u8; 16],
    pub dst_addr: [u8; 16],
}

pub const IPV6_MAX_EXTENSION_HEADER_COUNT: usize = 8;

pub const IP_PROTO_HOP_OPT: u8 = 0;
pub const IP_PROTO_IPV4: u8 = 4;
pub const IP_PROTO_TCP: u8 = 6;
pub const IP_PROTO_UDP: u8 = 17;
pub const IP_PROTO_IPV6: u8 = 41;
pub const IP_PROTO_IPV6_ROUTE: u8 = 43;
pub const IP_PROTO_IPV6_FRAG: u8 = 44;
pub const IP_PROTO_IPV6_OPTS: u8 = 60;
