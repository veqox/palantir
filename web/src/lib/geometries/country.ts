import { toCartesian } from "$lib/utils/geo";
import { glsl } from "$lib/utils/glsl";
import { Mesh, type OGLRenderingContext, Program, Color, Geometry, Vec3 } from "ogl";
import earcut from "earcut";

const fragment = glsl`#version 300 es
precision highp float;

in vec3 vNormal;

uniform vec3 color;

out vec4 fragColor;

void main() {
    vec3 normal = normalize(vNormal);
    float lighting = dot(normal, normalize(vec3(-0.5, 0.5, 0)));
    fragColor = vec4(color + lighting * 0.3, 3);
}`;

const vertex = glsl`#version 300 es
in vec3 position;
in vec3 normal;

uniform mat4 modelViewMatrix;
uniform mat4 projectionMatrix;
uniform mat3 normalMatrix;

out vec3 vNormal;

void main() {
    vNormal = normalize(normalMatrix * normal);
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`;

export class Country extends Mesh<Geometry, Program> {
	private static spherePoints: Vec3[];

	constructor(gl: OGLRenderingContext, polygons: number[][][][]) {
		const program = new Program(gl, {
			fragment,
			vertex,
			uniforms: {
				color: { value: new Color(Math.random() * 65_535) },
			},
		});

		const vertices: Vec3[] = [];

		for (const rings of polygons) {
			for (const ring of rings) {
				for (let i = 0; i < ring.length - 1; i++) {
					const current = toCartesian({ lon: ring[i][0], lat: ring[i][1] });
					const next = toCartesian({ lon: ring[i + 1][0], lat: ring[i + 1][1] });

					vertices.push(current.clone().multiply(1.05), current, next.clone().multiply(1.05), current, next, next.clone().multiply(1.05));
				}
			}
		}

		for (const rings of polygons) {
			for (const ring of rings) {
				const coords = ring.map(([lon, lat]) => toCartesian({ lon, lat }).multiply(1.05));

				const projected = projectToPlane(coords);
				const tris = earcut(projected.flat());

				for (const idx of tris) {
					vertices.push(coords[idx]);
				}
			}
		}

		const position = new Float32Array(vertices.flat());
		const normal = new Float32Array(vertices.flatMap((v) => v.clone().normalize()));

		const geometry = new Geometry(gl, {
			position: { size: 3, data: position },
			normal: { size: 3, data: normal },
		});

		super(gl, { program, geometry, mode: gl.TRIANGLES });
	}
}

function projectToPlane(polygon: Vec3[]): [number, number][] {
	const centroid = new Vec3(0, 0, 0);
	for (const v of polygon) centroid.add(v);
	centroid.divide(polygon.length);

	const normal = centroid.clone().normalize();
	const tangent = new Vec3().cross(normal, new Vec3(0, 1, 0));
	if (tangent.len() < 0.001) tangent.set(1, 0, 0);
	tangent.normalize();
	const bitangent = new Vec3().cross(normal, tangent);

	return polygon.map((v) => {
		const d = v.clone().sub(centroid);
		return [d.dot(tangent), d.dot(bitangent)];
	});
}
