export function glsl(strings: TemplateStringsArray, ...values: any[]) {
	return String.raw(strings, ...values).trim();
}
