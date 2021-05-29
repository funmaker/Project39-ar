#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;

layout(location = 0) out vec2 f_uv;
layout(location = 1) out vec3 f_normal;

layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

layout(push_constant) uniform Pc {
	mat4 model;
} pc;

void main() {
	mat4 mv = commons.view[gl_ViewIndex] * pc.model;
	mat4 mvp = commons.projection[gl_ViewIndex] * mv;
	mat3 normal_matrix = mat3(mv);
	
	gl_Position = mvp * vec4(pos, 1.0);
	
	f_uv = uv;
	f_normal = normalize(normal_matrix * normal);
}
