export interface Peer {
	addr: string;
	info: IpInfo;
	ingress_bytes: number;
	egress_bytes: number;
	last_message: Date;
}

export type IpInfo =
	| {
			lat: number;
			lon: number;
			country_code: string;
			source: "City";
			city_name: string;
			accuracy_radius: number;
	  }
	| {
			lat: number;
			lon: number;
			country_code: string;
			source: "Manual";
	  }
	| {
			lat: number;
			lon: number;
			country_code: string;
			source: "RegisteredCountry";
	  };

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
