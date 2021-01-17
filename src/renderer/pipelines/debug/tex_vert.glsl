#version 450

layout(location = 0) in vec3 pos;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec4 color;

layout(location = 0) out vec2 f_uv;
layout(location = 1) out vec4 f_color;

void main() {
	gl_Position = vec4(pos, 1.0);
	f_color = color;
	f_uv = uv;
}
