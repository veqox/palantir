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
				uThickness: { value: 5 },
				uColor: { value: new Color("#DE3163") },
			},
		});
	}

	private get segmentCount() {
		const indicesPerSegment = 6;
		return Math.floor((this.geometry.attributes.index?.count ?? 0) / indicesPerSegment);
	}

	private get indicesPerSegment() {
		return 6;
	}

	fadeIn(duration = 500) {
		const totalSegments = this.segmentCount;
		const start = performance.now();

		return new Promise<void>((resolve) => {
			const update = (now: number) => {
				const elapsed = now - start;
				const progress = Math.min(elapsed / duration, 1);

				const count = Math.max(
					this.indicesPerSegment,
					Math.min(Math.floor(totalSegments * progress) * this.indicesPerSegment, this.geometry.attributes.index!.count!),
				);

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
		const totalSegments = this.segmentCount;
		const start = performance.now();

		return new Promise<void>((resolve) => {
			const update = (now: number) => {
				const elapsed = now - start;
				const progress = Math.min(elapsed / duration, 1);

				const count = Math.max(
					this.indicesPerSegment,
					Math.min(Math.floor(totalSegments * (1 - progress)) * this.indicesPerSegment, this.geometry.attributes.index!.count!!),
				);

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
}
