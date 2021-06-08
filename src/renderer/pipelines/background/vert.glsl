#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec2 pos;

layout(location = 0) out vec2 f_uv;

//layout(push_constant) uniform Pc {
//	mat4 model;
//} pc;

void main() {
	gl_Position = vec4(pos, 0.0, 1.0);
	
	f_uv = pos / 2.0 + vec2(0.5);
}
