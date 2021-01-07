#version 450

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in float edge_scale;

layout(location = 0) out vec2 f_uv;

layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

layout(push_constant) uniform Pc {
	mat4 model;
	vec4 color;
	uint eye;
	float scale;
} pc;

void main() {
	mat4 mv = commons.view[pc.eye] * pc.model;
	mat4 mvp = commons.projection[pc.eye] * mv;
	mat3 normal_matrix = mat3(mv);
	
	vec3 view_normal = normalize(normal_matrix * normal);
	vec4 view_pos = mv * vec4(pos, 1.0);
	view_pos += vec4(view_normal * pc.scale * edge_scale * length(vec3(view_pos)), 0.0);
	
	gl_Position = commons.projection[pc.eye] * view_pos;
	
	f_uv = uv;
}
