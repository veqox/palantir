export class Peer {
	constructor(
		public addr: string,
		public info:
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
			  },
		public ingress_bytes: number,
		public egress_bytes: number,
		public last_message: Date,
	) {}

	get active() {
		return Date.now() - this.last_message.getTime() < 1000 * 60;
	}

	static fromJSON(obj: any): Peer {
		return new Peer(
			obj.addr,
			obj.info,
			obj.ingress_bytes,
			obj.egress_bytes,
			new Date(obj.last_message.secs_since_epoch * 1000 + obj.last_message.nanos_since_epoch / 1_000_000),
		);
	}
}

export class Packet {
	constructor(
		public proto: string,
		public src_addr: string,
		public src_location: Location,
		public dst_addr: string,
		public dst_location: Location,
		public bytes: number,
		public timestamp: Date,
	) {}

	static fromJSON(obj: any): Packet {
		return new Packet(
			obj.proto,
			obj.src_addr,
			obj.src_location,
			obj.dst_addr,
			obj.dst_location,
			obj.bytes,
			new Date(obj.timestamp.secs_since_epoch * 1000 + obj.timestamp.nanos_since_epoch / 1_000_000),
		);
	}
}

export type Event = { peer: Peer; packet?: never } | { packet: Packet; peer?: never };
