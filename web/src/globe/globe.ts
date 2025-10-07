import { Mesh, type OGLRenderingContext, Program, Color, Sphere } from "ogl";
import fragment from "./fragment.glsl?raw";
import vertex from "./vertex.glsl?raw";

export class Globe {
	private static program: Program;
	mesh: Mesh;

	constructor(gl: OGLRenderingContext) {
		const program =
			Globe.program ??
			(Globe.program = new Program(gl, {
				fragment: fragment,
				vertex: vertex,
				uniforms: {
					color: { value: new Color("#343fac") },
				},
			}));

		const geometry = new Sphere(gl, {
			radius: 1,
			widthSegments: 360,
			heightSegments: 181,
		});

		this.mesh = new Mesh(gl, { geometry, program });
	}
}
