#version 450

layout(location = 0) in vec2 f_uv;
layout(location = 1) in vec3 f_normal;

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
	uint eye;
} pc;

void main() {
	vec3 light_direction = commons.light_direction[pc.eye].xyz;
	float lambert = max(dot(-f_normal, light_direction), 0.0);
	float light = lambert * (1.0 - commons.ambient) + commons.ambient;
	
	o_color = texture(tex, f_uv) * vec4(light, light, light, 1.0);
}
