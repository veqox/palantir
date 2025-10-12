#[repr(C, packed)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[derive(Debug, Copy, Clone)]
pub struct EthHdr {
    pub dst_addr: [u8; 6],
    pub src_addr: [u8; 6],
    pub ether_type: u16,
}

pub const ETHER_TYPE_IPV4: u16 = 0x0800;
pub const ETHER_TYPE_IPV6: u16 = 0x86DD;
