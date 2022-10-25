#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec2 f_uv;

layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 1) uniform sampler2DArray tex;

layout(push_constant) uniform Pc {
	mat4 model;
	float ratio;
	float frame;
} pc;

void main() {
	o_color = texture(tex, vec3(f_uv, pc.frame));
}
