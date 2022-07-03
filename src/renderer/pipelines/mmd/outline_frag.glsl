#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec2 f_uv;

layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

layout(set = 1, binding = 0) uniform sampler2D tex;

layout(push_constant) uniform Pc {
	mat4 model;
	vec4 color;
	float scale;
} pc;

void main() {
	// TODO: required to prevent optimizing the binding away for pipeline compatibility. Maybe use separate bindings for edge pipeline?
	o_color.a = commons.ambient;

	o_color = pc.color;
	o_color.a *= texture(tex, f_uv).a;

	o_color.rgb *= o_color.a; // Premultiply alpha
}
