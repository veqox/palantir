#version 300 es
precision highp float;

in vec3 vNormal;

uniform vec3 color;

out vec4 fragColor;

void main() {
    vec3 normal = normalize(vNormal);
    float lighting = dot(normal, normalize(vec3(-0.5, 0.5, 0)));
    fragColor = vec4(color + lighting * 0.3, 3);
}
