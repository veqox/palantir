#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TcpHdr {
    pub src: [u8; 2],
    pub dst: [u8; 2],
    pub seq: [u8; 4],
    pub ack_seq: [u8; 4],
    pub data_offset_flags: [u8; 2],
    pub window: [u8; 2],
    pub check: [u8; 2],
    pub urg_ptr: [u8; 2],
}
