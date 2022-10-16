#version 450
#extension GL_EXT_multiview : require

layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

layout(set = 0, binding = 1) uniform sampler2D tex;

layout(push_constant) uniform Pc {
	mat4 model;
	vec4 color;
	float scale;
} pc;

void main() {
	o_color = texture(tex, vec2(0.0, 0.0)) * commons.ambient;
	o_color = pc.color;
}
