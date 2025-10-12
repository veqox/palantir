import "./style.css";

// https://www.naturalearthdata.com/downloads/110m-physical-vectors
// converted to geoJson
import land from "./assets/land.json";

import { Renderer, Camera, Transform, Vec3, Orbit, Polyline } from "ogl";
import { geoToCartesian } from "./utils";
import { Beacon } from "./beacon/beacon";
import { Globe } from "./globe/globe";
import { Trace } from "./trace/trace";

const renderer = new Renderer({ dpr: 2, webgl: 2 });

const gl = renderer.gl;

document.body.appendChild(gl.canvas);

const camera = new Camera(gl, { fov: 40 });
camera.position.set(3, 0, 4);

const controls = new Orbit(camera, { target: new Vec3() });

function resize() {
	renderer.setSize(window.innerWidth, window.innerHeight);
	camera.perspective({ aspect: gl.canvas.width / gl.canvas.height });
}
window.addEventListener("resize", resize, false);
resize();

const scene = new Transform();

const globe = new Globe(gl);
globe.mesh.setParent(scene);

for (const polygon of land.geometries) {
	for (const coordinates of polygon.coordinates) {
		const points = coordinates.map(([lon, lat]) => geoToCartesian({ lon, lat }).scale(1.0001));

		const geometry = new Polyline(gl, {
			points,
		});

		geometry.mesh.setParent(scene);
	}
}

const beacons = new Set<string>();

{
	const eventSource = new EventSource("http://localhost:3000/events");

	eventSource.onmessage = async (e) => {
		const { src_addr, dst_addr, src_location, dst_location } = JSON.parse(e.data, (key, value) => {
			if (key === "ts") {
				return new Date(value.secs_since_epoch * 1000 + value.nanos_since_epoch / 1_000_000);
			}
			return value;
		});

		const trace = new Trace(gl, { from: geoToCartesian(src_location), to: geoToCartesian(dst_location) });
		trace.setParent(scene);

		if (!beacons.has(src_addr)) {
			let beacon = new Beacon(gl, { position: geoToCartesian(src_location), height: 0.3 });
			beacon.setParent(scene);
			beacon.animate();

			beacons.add(src_addr);
		}

		await trace.fadeIn();

		if (!beacons.has(dst_addr)) {
			let beacon = new Beacon(gl, { position: geoToCartesian(dst_location), height: 0.3 });
			beacon.setParent(scene);
			beacon.animate();

			beacons.add(dst_addr);
		}

		await trace.fadeOut();
		trace.setParent(null);
	};
}

function update() {
	requestAnimationFrame(update);
	renderer.render({ scene, camera });
	controls.update();
}
update();
