import { Path, Tube, Mesh, type OGLRenderingContext, Program, Vec3, Color, Transform, Sphere, Raycast } from "ogl";
import { glsl } from "$lib/utils/glsl";

type BeaconOptions = {
	position: Vec3;
	height: number;
};

const fragment = glsl`#version 300 es
precision highp float;

uniform vec3 color;

out vec4 fragColor;

void main() {
    fragColor = vec4(color, 1);
}`;

const vertex = glsl`#version 300 es
precision highp float;

uniform mat4 modelViewMatrix;
uniform mat4 projectionMatrix;

in vec3 position;

void main() {
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`;

export class Beacon extends Transform {
	private static programs: Record<string, Program> = {};
	private t: number = 0;
	private meshes: Record<string, Mesh> = {};

	constructor(gl: OGLRenderingContext, { position = new Vec3(), height = 1 }: Partial<BeaconOptions> = {}) {
		super();

		const path = new Path();
		path.moveTo(position);
		path.lineTo(position.clone().add(position.clone().normalize().multiply(height)));

		const line = (this.meshes.line = new Mesh(gl, {
			geometry: new Tube(gl, {
				radius: 0.005,
				path,
			}),
			program:
				Beacon.programs.line ??
				(Beacon.programs.line = new Program(gl, {
					fragment,
					vertex,
					uniforms: {
						color: { value: new Color("#61afef") },
					},
				})),
		}));

		this.addChild(line);

		const sphere = (this.meshes.sphere = new Mesh(gl, {
			geometry: new Sphere(gl, { radius: 0.01 }),
			program:
				Beacon.programs.sphere ??
				(Beacon.programs.sphere = new Program(gl, {
					fragment,
					vertex,
					uniforms: {
						color: { value: new Color("#FFF") },
					},
				})),
		}));

		sphere.position.set(position.clone().add(position.clone().normalize().multiply(height)));

		this.addChild(sphere);
	}

	fadeIn() {
		const line = this.meshes.line.geometry as Tube;
		const step = Math.min(Math.round(line.indices.length / 60), line.indices.length - this.t);
		this.meshes.sphere.visible = false;

		return new Promise<void>((resolve) => {
			const update = () => {
				this.t += step;

				if (this.t < line.indices.length) {
					line.setDrawRange(0, this.t);
					requestAnimationFrame(update);
				} else {
					this.meshes.sphere.visible = true;
					resolve();
				}
			};
			update();
		});
	}

	fadeOut() {
		const line = this.meshes.line.geometry as Tube;
		const step = Math.min(Math.round(line.indices.length / 60), line.indices.length - this.t);
		this.meshes.sphere.visible = false;

		return new Promise<void>((resolve) => {
			const update = () => {
				this.t += step;

				if (this.t < line.indices.length * 2) {
					line.setDrawRange(this.t - line.indices.length, line.indices.length - (this.t - line.indices.length));
					requestAnimationFrame(update);
				} else {
					resolve();
				}
			};
			update();
		});
	}
}
