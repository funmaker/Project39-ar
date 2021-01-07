#version 450

layout(location = 0) in vec2 f_uv;

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
	uint eye;
	float scale;
} pc;

void main() {
	o_color = pc.color;
	o_color.a *= texture(tex, f_uv).a;
	
	o_color.rgb *= o_color.a; // Premultiply alpha
}
