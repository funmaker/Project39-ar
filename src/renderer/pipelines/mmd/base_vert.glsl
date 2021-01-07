#version 450

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in float edge_scale;

layout(location = 0) out vec3 f_pos;
layout(location = 1) out vec2 f_uv;
layout(location = 2) out vec3 f_normal;

layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

layout(push_constant) uniform Pc {
	mat4 model;
	uint eye;
} pc;

void main() {
	mat4 mv = commons.view[pc.eye] * pc.model;
	mat4 mvp = commons.projection[pc.eye] * mv;
	mat3 normal_matrix = mat3(mv);
	
	gl_Position = mvp * vec4(pos, 1.0);
	
	f_pos = vec3(mv * vec4(pos, 1.0));
	f_uv = uv;
	f_normal = normalize(normal_matrix * normal);
}
