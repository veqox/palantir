import { Path, Tube, Mesh, type OGLRenderingContext, Program, Vec3, Color, Transform, Sphere } from "ogl";
import fragment from "./fragment.glsl?raw";
import vertex from "./vertex.glsl?raw";

type BeaconOptions = {
	position: Vec3;
	height: number;
};

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

	animate() {
		const line = this.meshes.line.geometry as Tube;
		const step = Math.min(Math.round(line.indices.length / 60), line.indices.length - this.t);
		this.meshes.sphere.visible = false;

		const update = () => {
			this.t += step;

			if (this.t < line.indices.length) {
				line.setDrawRange(0, this.t);
				requestAnimationFrame(update);
			} else {
				this.meshes.sphere.visible = true;
			}
		};
		update();
	}
}
