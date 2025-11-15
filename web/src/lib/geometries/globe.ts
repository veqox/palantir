import geoData from "$lib/assets/countries.json";
import { toCartesian, toLatLon } from "$lib/utils/geo";
import { glsl } from "$lib/utils/glsl";
import { Mesh, type OGLRenderingContext, Program, Geometry, Vec3, Color } from "ogl";

const vertex = glsl`#version 300 es
in vec3 position;
in vec3 color;
in float extrusion;

uniform mat4 modelViewMatrix;
uniform mat4 projectionMatrix;

out vec4 vMVPos;
out vec3 vColor;

void main() {
	vMVPos = modelViewMatrix * vec4(position * extrusion, 1.0);
	vColor = color;
	gl_Position = projectionMatrix * vMVPos;
}`;

const fragment = glsl`#version 300 es
precision highp float;

in vec4 vMVPos;
in vec3 vColor;
out vec4 fragColor;

vec3 normals(vec3 pos) {
	vec3 fdx = dFdx(pos);
    vec3 fdy = dFdy(pos);
    return normalize(cross(fdx, fdy));
}

void main() {
	vec3 normal = normals(vMVPos.xyz);
    fragColor = vec4(vColor + (normal * 0.3), 1.0);
}`;

export class Globe extends Mesh<Geometry, Program> {
	private countryFaces: Map<string, number[][]>;
	private faces: number[][];
	private vertices: Vec3[];

	constructor(gl: OGLRenderingContext) {
		const program = new Program(gl, {
			fragment,
			vertex,
		});

		const t = (1 + Math.sqrt(5)) / 2;

		let vertices = [
			new Vec3(-1, t, 0),
			new Vec3(1, t, 0),
			new Vec3(-1, -t, 0),
			new Vec3(1, -t, 0),
			new Vec3(0, -1, t),
			new Vec3(0, 1, t),
			new Vec3(0, -1, -t),
			new Vec3(0, 1, -t),
			new Vec3(t, 0, -1),
			new Vec3(t, 0, 1),
			new Vec3(-t, 0, -1),
			new Vec3(-t, 0, 1),
		].map((v) => v.normalize());

		let faces = [
			[0, 11, 5],
			[0, 5, 1],
			[0, 1, 7],
			[0, 7, 10],
			[0, 10, 11],
			[1, 5, 9],
			[5, 11, 4],
			[11, 10, 2],
			[10, 7, 6],
			[7, 1, 8],
			[3, 9, 4],
			[3, 4, 2],
			[3, 2, 6],
			[3, 6, 8],
			[3, 8, 9],
			[4, 9, 5],
			[2, 4, 11],
			[6, 2, 10],
			[8, 6, 7],
			[9, 8, 1],
		];

		for (let i = 0; i < 7; i++) {
			const cache: Map<string, number> = new Map();

			const subdivided = [];

			const midpoint = (a: number, b: number): number => {
				const key = a < b ? `${a}-${b}` : `${b}-${a}`;

				let i = cache.get(key);
				if (i) return i;

				const mid = vertices[a].clone().add(vertices[b]).normalize();
				i = vertices.push(mid) - 1;
				cache.set(key, i);

				return i;
			};

			for (const face of faces) {
				const [a, b, c] = face;
				const ab = midpoint(a, b);
				const bc = midpoint(b, c);
				const ca = midpoint(c, a);

				subdivided.push([a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]);
			}

			faces = subdivided;
		}

		const bounds = (coordinates: number[][][][]) => {
			let minLat = Infinity,
				maxLat = -Infinity,
				minLon = Infinity,
				maxLon = -Infinity;

			for (const polygons of coordinates) {
				for (const ring of polygons) {
					for (const [lon, lat] of ring) {
						if (lon < minLon) minLon = lon;
						if (lon > maxLon) maxLon = lon;
						if (lat < minLat) minLat = lat;
						if (lat > maxLat) maxLat = lat;
					}
				}
			}

			return { minLat, maxLat, minLon, maxLon };
		};

		type Country = {
			properties: {
				name: string;
				iso_code: string;
			};
			bounds: {
				minLat: number;
				maxLat: number;
				minLon: number;
				maxLon: number;
			};
			geometry: {
				coordinates: number[][][][];
			};
		};

		const countries = geoData.features.map((f) => {
			const coordinates = (f.geometry.type === "MultiPolygon" ? f.geometry.coordinates : [f.geometry.coordinates]) as number[][][][];

			return {
				properties: { name: f.properties.NAME, iso_code: f.properties.ISO_A2 },
				bounds: bounds(coordinates),
				geometry: {
					coordinates,
				},
			};
		});

		const isLand = ({ lon, lat }: { lon: number; lat: number }, { bounds, geometry }: Country) => {
			if (lon < bounds.minLon || lon > bounds.maxLon || lat < bounds.minLat || lat > bounds.maxLat) return false;
			for (const rings of geometry.coordinates) {
				for (const ring of rings) {
					if (isPointInPolygon({ lon, lat }, ring)) return true;
				}
			}
			return false;
		};

		const isPointInPolygon = ({ lon, lat }: { lon: number; lat: number }, ring: number[][]) => {
			let inside = false;
			for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
				const [cLon, cLat] = ring[i];
				const [nLon, nLat] = ring[j];

				const intersect = cLat > lat !== nLat > lat && lon < ((nLon - cLon) * (lat - cLat)) / (nLat - cLat + 1e-12) + cLon;

				if (intersect) inside = !inside;
			}
			return inside;
		};

		const extrusions: Array<number> = new Array(vertices.length).fill(1);
		const colors: Array<Color> = new Array(vertices.length).fill(new Color("#2D68C4"));
		const countryFaces: Map<string, number[][]> = new Map();

		for (const face of faces) {
			const centroid = new Vec3();
			face.forEach((i) => centroid.add(vertices[i]));
			centroid.scale(1 / 3).normalize();

			const { lat, lon } = toLatLon(centroid);

			for (const country of countries) {
				if (isLand({ lon, lat }, country)) {
					countryFaces.set(country.properties.iso_code, [...(countryFaces.get(country.properties.iso_code) ?? []), face]);

					for (const i of face) {
						extrusions[i] = 1.05;
						colors[i] = new Color("#008000");
					}
					break;
				}
			}
		}

		const geometry = new Geometry(gl, {
			position: { size: 3, data: new Float32Array(vertices.flat()) },
			index: { data: new Uint32Array(faces.flat()) },
			color: { size: 3, data: new Float32Array(colors.flat()) },
			extrusion: { data: new Float32Array(extrusions.flat()) },
		});

		super(gl, { program, geometry });

		this.countryFaces = countryFaces;
		this.faces = faces;
		this.vertices = vertices;
	}

	selectCountry(iso_code: string, duration: number = 500, extrusion: number = 0.01) {
		const faces = this.countryFaces.get(iso_code);
		if (!faces) return;

		const start = performance.now();

		return new Promise<void>((resolve) => {
			const update = (now: number) => {
				const elapsed = now - start;
				const progress = Math.min(elapsed / duration, 1);

				const count = progress * extrusion;

				for (const face of faces) {
					for (const i of face) {
						this.geometry.attributes.extrusion.data![i] = 1.05 + count;
						const color = new Color("#880000");
						this.geometry.attributes.color.data![i * 3] = color[0];
						this.geometry.attributes.color.data![i * 3 + 1] = color[1];
						this.geometry.attributes.color.data![i * 3 + 2] = color[2];
					}
				}

				this.geometry.attributes.extrusion.needsUpdate = true;
				this.geometry.attributes.color.needsUpdate = true;

				if (progress < 1) {
					requestAnimationFrame(update);
				} else {
					resolve();
				}
			};
			requestAnimationFrame(update);
		});
	}

	unselectCountry(iso_code: string, duration: number = 500, extrusion: number = 0.01) {
		const faces = this.countryFaces.get(iso_code);
		if (!faces) return;

		const start = performance.now();

		return new Promise<void>((resolve) => {
			const update = (now: number) => {
				const elapsed = now - start;
				const progress = Math.min(elapsed / duration, 1);

				const count = progress * extrusion;
				const color = new Color("#008000");

				for (const face of faces) {
					for (const i of face) {
						this.geometry.attributes.extrusion.data![i] = 1.06 - count;
						this.geometry.attributes.color.data![i * 3] = color[0];
						this.geometry.attributes.color.data![i * 3 + 1] = color[1];
						this.geometry.attributes.color.data![i * 3 + 2] = color[2];
					}
				}

				this.geometry.attributes.extrusion.needsUpdate = true;
				this.geometry.attributes.color.needsUpdate = true;

				if (progress < 1) {
					requestAnimationFrame(update);
				} else {
					resolve();
				}
			};
			requestAnimationFrame(update);
		});
	}

	selectRegion({ lat, lon }: { lat: number; lon: number }, radius: number = 1, duration: number = 500, extrusion: number = 0.01) {
		const radiusRad = (Math.PI / 180) * radius;
		const threshold = Math.cos(radiusRad);
		const center = toCartesian({ lat, lon });

		const region = this.faces.filter((f) => {
			const a = this.vertices[f[0]];
			const b = this.vertices[f[1]];
			const c = this.vertices[f[2]];

			const centroid = a
				.clone()
				.add(b)
				.add(c)
				.scale(1 / 3)
				.normalize();

			return center.dot(centroid) >= threshold;
		});

		const start = performance.now();
		return new Promise<void>((resolve) => {
			const update = (now: number) => {
				const elapsed = now - start;
				const progress = Math.min(elapsed / duration, 1);
				const extrudeValue = 1.05 + progress * extrusion;
				const color = new Color("#AA0000");

				for (const face of region) {
					for (const i of face) {
						this.geometry.attributes.extrusion.data![i] = extrudeValue;
						this.geometry.attributes.color.data![i * 3] = color.r;
						this.geometry.attributes.color.data![i * 3 + 1] = color.g;
						this.geometry.attributes.color.data![i * 3 + 2] = color.b;
					}
				}

				this.geometry.attributes.extrusion.needsUpdate = true;
				this.geometry.attributes.color.needsUpdate = true;

				if (progress < 1) requestAnimationFrame(update);
				else resolve();
			};

			requestAnimationFrame(update);
		});
	}

	unselectRegion({ lat, lon }: { lat: number; lon: number }, radius: number = 1, duration: number = 500, extrusion: number = 0.01) {
		const radiusRad = (Math.PI / 180) * radius;
		const threshold = Math.cos(radiusRad);
		const center = toCartesian({ lat, lon });

		const region = this.faces.filter((f) => {
			const a = this.vertices[f[0]];
			const b = this.vertices[f[1]];
			const c = this.vertices[f[2]];

			const centroid = a
				.clone()
				.add(b)
				.add(c)
				.scale(1 / 3)
				.normalize();

			return center.dot(centroid) >= threshold;
		});

		const start = performance.now();
		return new Promise<void>((resolve) => {
			const update = (now: number) => {
				const elapsed = now - start;
				const progress = Math.min(elapsed / duration, 1);
				const count = progress * extrusion;
				const color = new Color("#008000");

				for (const face of region) {
					for (const i of face) {
						this.geometry.attributes.extrusion.data![i] = 1.06 - count;
						this.geometry.attributes.color.data![i * 3] = color.r;
						this.geometry.attributes.color.data![i * 3 + 1] = color.g;
						this.geometry.attributes.color.data![i * 3 + 2] = color.b;
					}
				}

				this.geometry.attributes.extrusion.needsUpdate = true;
				this.geometry.attributes.color.needsUpdate = true;

				if (progress < 1) requestAnimationFrame(update);
				else resolve();
			};

			requestAnimationFrame(update);
		});
	}
}
