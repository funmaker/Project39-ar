#version 450
#extension GL_EXT_multiview : require

layout(location = 1) in vec2 pos;

layout(location = 0) out vec2 f_uv;

layout(push_constant) uniform Pc {
	mat4 model;
	vec4 uv_transform;
} pc;

layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

void main() {
	mat4 mvp = commons.projection[gl_ViewIndex] * commons.view[gl_ViewIndex] * pc.model;
	
	gl_Position = mvp * vec4(pos, 0.0, 1.0);
	f_uv = (-pos / 2.0 + vec2(0.5)) * pc.uv_transform.xy + pc.uv_transform.zw;
}
