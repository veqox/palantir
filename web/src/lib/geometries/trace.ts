import { midpoint } from "$lib/utils/geo";
import { glsl } from "$lib/utils/glsl";
import { Color, Mesh, Path, Program, Tube, Vec3, type OGLRenderingContext } from "ogl";

type TraceOptions = { from: Vec3; to: Vec3; color: Color };

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

export class Trace extends Mesh<Tube, Program> {
	private static program: Program;

	private t: number = 0;

	constructor(gl: OGLRenderingContext, { from = new Vec3(), to = new Vec3(), color = new Color() }: Partial<TraceOptions> = {}) {
		const program =
			Trace.program ??
			(Trace.program = new Program(gl, {
				fragment,
				vertex,
				uniforms: {
					color: { value: color },
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

	fadeIn(duration = 500) {
		const total = this.geometry.indices.length;
		const start = performance.now();

		return new Promise<void>((resolve) => {
			const update = (now: number) => {
				const elapsed = now - start;
				const progress = Math.min(elapsed / duration, 1);
				this.t = Math.floor(total * progress);
				this.geometry.setDrawRange(0, this.t);

				if (progress < 1) {
					requestAnimationFrame(update);
				} else {
					resolve();
				}
			};
			requestAnimationFrame(update);
		});
	}

	fadeOut(duration = 500) {
		const total = this.geometry.indices.length;
		const start = performance.now();

		return new Promise<void>((resolve) => {
			const update = (now: number) => {
				const elapsed = now - start;
				const progress = Math.min(elapsed / duration, 1);
				const visible = Math.floor(total * (1 - progress));
				this.geometry.setDrawRange(0, visible);

				if (progress < 1) {
					requestAnimationFrame(update);
				} else {
					resolve();
				}
			};
			requestAnimationFrame(update);
		});
	}
}
