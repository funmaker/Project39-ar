#version 450
#extension GL_EXT_multiview : require

layout(location = 2) in vec2 pos;

layout(location = 0) out vec2 f_uv;

layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

layout(push_constant) uniform Pc {
	mat4 model;
	float ratio;
	float frame;
} pc;

void main() {
	mat4 mvp = commons.projection[gl_ViewIndex] * commons.view[gl_ViewIndex] * pc.model;
	
	gl_Position = mvp * vec4(pos * vec2(pc.ratio, 1.0), 0.0, 1.0);
	f_uv = -pos / 2.0 + vec2(0.5);
}
