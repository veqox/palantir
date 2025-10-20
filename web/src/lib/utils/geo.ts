import { Vec3 } from "ogl";

export function toCartesian({ lat, lon }: { lat: number; lon: number }): Vec3 {
	const phi = ((90 - lat) * Math.PI) / 180;
	const theta = ((lon + 180) * Math.PI) / 180;

	const x = -(Math.sin(phi) * Math.cos(theta));
	const z = Math.sin(phi) * Math.sin(theta);
	const y = Math.cos(phi);

	return new Vec3(x, y, z);
}

export function midpoint(start: Vec3, end: Vec3): Vec3 {
	return start.clone().add(end).divide(2).normalize();
}
