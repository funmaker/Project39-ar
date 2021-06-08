#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec2 f_uv;

layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 0) uniform sampler2D tex;

//layout(push_constant) uniform Pc {
//	mat4 model;
//} pc;

void main() {
	vec2 uv = f_uv;
	uv.x = uv.x / 2.0 + gl_ViewIndex;
	
	o_color = texture(tex, uv);
}
