#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;

layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

layout(push_constant) uniform Pc {
	mat4 model;
	vec4 color;
	float scale;
} pc;

void main() {
	mat4 mv = commons.view[gl_ViewIndex] * pc.model;
	mat4 mvp = commons.projection[gl_ViewIndex] * mv;
	
	gl_Position = mvp * vec4(pos + normalize(pos) * pc.scale, 1.0);
}
