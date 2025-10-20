export interface Peer {
	addr: string;
	info: IpInfo;
	ingress_bytes: number;
	egress_bytes: number;
	last_message: Date;
}

export interface IpInfo {
	lat: number;
	lon: number;
	country: string;
	region: string;
	city: string;
	isp: string;
	mobile: boolean;
	proxy: boolean;
	hosting: boolean;
}

export interface Packet {
	proto: string;
	src_addr: string;
	src_location: Location;
	dst_addr: string;
	dst_location: Location;
	bytes: number;
	timestamp: Date;
}

export type Event = { peer: Peer; packet?: never } | { packet: Packet; peer?: never };
