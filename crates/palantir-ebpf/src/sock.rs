use core::ffi::c_void;

pub const AF_INET: u8 = 2;
pub const AF_INET6: u8 = 10;

#[repr(C)]
pub struct Sock {
    pub sock_common: SockCommon,
}

#[repr(C)]
pub struct SockCommon {
    pub daddr: u32,
    pub rcv_saddr: u32,
    pub hash: u32,
    pub dport: u16,
    pub num: u16,
    pub family: u8,
    pub state: u8,
    pub reuse: bool,
    pub reuseport: bool,
    pub ipv6only: bool,
    pub net_refcnt: bool,
    pub bound_dev_if: i32,
    pub prot: *const c_void,
    pub net: *const c_void,
    pub v6_daddr: [u8; 16],
    pub v6_rcv_saddr: [u8; 16],
}
