import land from "$lib/assets/land.json";
import { toCartesian } from "$lib/utils/geo";
import { glsl } from "$lib/utils/glsl";
import { Mesh, type OGLRenderingContext, Program, Color, Sphere, Transform, Polyline } from "ogl";

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

export class Globe extends Transform {
	constructor(gl: OGLRenderingContext) {
		super();

		const program = new Program(gl, {
			fragment,
			vertex,
			uniforms: {
				color: { value: new Color("#343fac") },
			},
		});

		const geometry = new Sphere(gl, {
			radius: 1,
			widthSegments: 360,
			heightSegments: 181,
		});

		const sphere = new Mesh(gl, { geometry, program });

		this.addChild(sphere);

		for (const polygon of land.geometries) {
			for (const coordinates of polygon.coordinates) {
				const points = coordinates.map(([lon, lat]) => toCartesian({ lon, lat }).scale(1.0001));

				const geometry = new Polyline(gl, {
					points,
				});

				this.addChild(geometry.mesh);
			}
		}
	}
}
