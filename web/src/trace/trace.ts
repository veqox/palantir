import { Color, Mesh, Path, Program, Tube, Vec3, type OGLRenderingContext } from "ogl";
import fragment from "./fragment.glsl?raw";
import vertex from "./vertex.glsl?raw";
import { midpoint } from "../utils";

type TraceOptions = { from: Vec3; to: Vec3 };

export class Trace extends Mesh<Tube, Program> {
	private static program: Program;
	private t: number = 0;

	constructor(gl: OGLRenderingContext, { from = new Vec3(), to = new Vec3() }: Partial<TraceOptions> = {}) {
		const program =
			Trace.program ??
			(Trace.program = new Program(gl, {
				fragment,
				vertex,
				uniforms: {
					color: { value: new Color("#df64b0") },
				},
			}));

		const mid = midpoint(from, to);
		const c1 = midpoint(from, mid).scale(1.4);
		const c2 = midpoint(mid, to).scale(1.4);

		const path = new Path();
		path.moveTo(from);
		path.bezierCurveTo(c1, c2, to);

		const geometry = new Tube(gl, {
			radius: 0.005,
			closed: false,
			path,
		});

		super(gl, { geometry, program });
	}

	fadeIn() {
		const step = Math.round(this.geometry.indices.length / 60);

		return new Promise<void>((resolve) => {
			const update = () => {
				if (this.t < this.geometry.indices.length) {
					this.geometry.setDrawRange(0, this.t);
					this.t += Math.min(step, this.geometry.indices.length - this.t);
					requestAnimationFrame(update);
				} else {
					resolve();
				}
			};
			update();
		});
	}

	fadeOut() {
		const step = Math.round(this.geometry.indices.length / 60);

		return new Promise<void>((resolve) => {
			const update = () => {
				if (this.t < this.geometry.indices.length * 2) {
					this.geometry.setDrawRange(
						this.t - this.geometry.indices.length,
						this.geometry.indices.length - (this.t - this.geometry.indices.length),
					);
					this.t += step;
					requestAnimationFrame(update);
				} else {
					resolve();
				}
			};
			update();
		});
	}
}
