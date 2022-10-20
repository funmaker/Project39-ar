#version 450

layout(location = 0) in vec2 f_uv;

layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 0) uniform sampler2D tex;

void main() {
	o_color = texture(tex, f_uv);
}
