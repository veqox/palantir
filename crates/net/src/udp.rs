#[repr(C)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[derive(Debug, Copy, Clone)]
pub struct UdpHdr {
    pub src: [u8; 2],
    pub dst: [u8; 2],
    pub len: [u8; 2],
    pub check: [u8; 2],
}
