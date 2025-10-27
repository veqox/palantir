import { midpoint } from "$lib/utils/geo";
import { Color, Path, Polyline, Vec3, type OGLRenderingContext } from "ogl";

type TraceOptions = { from: Vec3; to: Vec3 };

export class Trace extends Polyline {
	constructor(gl: OGLRenderingContext, { from = new Vec3(), to = new Vec3() }: Partial<TraceOptions> = {}) {
		const mid = midpoint(from, to);
		const c1 = midpoint(from, mid).scale(1.4);
		const c2 = midpoint(mid, to).scale(1.4);

		const path = new Path();
		path.moveTo(from);
		path.bezierCurveTo(c1, c2, to);

		super(gl, {
			points: path.getPoints(256),
			uniforms: {
				uThickness: { value: 3 },
				uColor: { value: new Color("#00D390") },
			},
		});
	}

	fadeIn(duration = 500) {
		const total = this.geometry.drawRange.count;
		const start = performance.now();

		return new Promise<void>((resolve) => {
			const update = (now: number) => {
				const elapsed = now - start;
				const progress = Math.min(elapsed / duration, 1);

				const count = Math.max(1, Math.min(Math.floor(total * progress), total));
				this.geometry.setDrawRange(0, count);

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
		const total = this.geometry.drawRange.count;
		const start = performance.now();

		return new Promise<void>((resolve) => {
			const update = (now: number) => {
				const elapsed = now - start;
				const progress = Math.min(elapsed / duration, 1);

				const visible = Math.min(Math.floor(total * (1 - progress)), total);
				if (visible > 0) this.geometry.setDrawRange(0, visible);

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
