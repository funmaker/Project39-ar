#version 450

layout(location = 0) in vec2 uv;
layout(location = 1) in vec3 normal;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D tex;
layout(set = 0, binding = 1) uniform sampler2D toon;
layout(set = 0, binding = 2) uniform sampler2D sphere;

vec3 light = vec3(-0.57735, -0.57735, -0.57735);
float ambient = 0.25;

void main() {
	float exposure = max(dot(normal, light), 0.0) * (1.0 - ambient) + ambient;
	
	f_color = texture(tex, uv) * vec4(exposure, exposure, exposure, 1.0);
}
