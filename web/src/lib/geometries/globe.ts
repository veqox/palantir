import land from "$lib/assets/countries.json";
import { toLatLon } from "$lib/utils/geo";
import { glsl } from "$lib/utils/glsl";
import { Mesh, type OGLRenderingContext, Program, Geometry, Vec3, Color } from "ogl";

const vertex = glsl`#version 300 es
in vec3 position;
in vec3 normal;
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

		for (let i = 0; i < 6; i++) {
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

		const extrusions: Array<number> = new Array(vertices.length).fill(1);

		const bounds = (ring: number[][]) => {
			let minLat = Infinity,
				maxLat = -Infinity,
				minLon = Infinity,
				maxLon = -Infinity;

			for (const [lon, lat] of ring) {
				if (lon < minLon) minLon = lon;
				if (lon > maxLon) maxLon = lon;
				if (lat < minLat) minLat = lat;
				if (lat > maxLat) maxLat = lat;
			}
			return { minLat, maxLat, minLon, maxLon };
		};

		const polygons = land.geometries
			.map((geometry) =>
				geometry.type === "MultiPolygon" ? (geometry.coordinates as number[][][][]) : [geometry.coordinates as unknown as number[][][]],
			)
			.map((coordinates) => coordinates.map((rings) => rings.map((ring) => ({ bounds: bounds(ring), ring }))));

		const isLand = (
			{ lon, lat }: { lon: number; lat: number },
			polygons: {
				bounds: {
					minLat: number;
					maxLat: number;
					minLon: number;
					maxLon: number;
				};
				ring: number[][];
			}[],
		) => {
			for (const { ring, bounds } of polygons) {
				if (lon < bounds.minLon || lon > bounds.maxLon || lat < bounds.minLat || lat > bounds.maxLat) continue;
				if (pointInPolygon({ lon, lat }, ring)) return true;
			}
			return false;
		};

		const pointInPolygon = ({ lon, lat }: { lon: number; lat: number }, ring: number[][]) => {
			let inside = false;
			for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
				const [cLon, cLat] = ring[i];
				const [nLon, nLat] = ring[j];

				const intersect = cLat > lat !== nLat > lat && lon < ((nLon - cLon) * (lat - cLat)) / (nLat - cLat + 1e-12) + cLon;

				if (intersect) inside = !inside;
			}
			return inside;
		};

		const normals: Array<Vec3> = new Array(vertices.length);
		const colors: Array<Color> = new Array(vertices.length).fill(new Color("#2D68C4"));

		outer: for (const face of faces) {
			const centroid = new Vec3();
			face.forEach((i) => centroid.add(vertices[i]));
			centroid.scale(1 / 3).normalize();

			normals.push(centroid, centroid, centroid);

			const { lat, lon } = toLatLon(centroid);

			for (const polygon of polygons) {
				for (const rings of polygon) {
					const inside = isLand({ lon, lat }, rings);

					if (inside) {
						for (const i of face) {
							extrusions[i] = 1.05;
							colors[i] = new Color("#008000");
						}
						continue outer;
					}
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
	}
}
