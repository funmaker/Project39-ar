#version 450

layout(location = 0) in vec2 pos;

layout(location = 0) out vec2 f_uv;

layout(push_constant) uniform Pc {
	vec2 scale;
} pc;

void main() {
	gl_Position = vec4(pc.scale * pos, -0.0, 1.0);
	f_uv = -pos / 2.0 + vec2(0.5);
}
