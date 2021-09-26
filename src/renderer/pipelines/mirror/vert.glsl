#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec3 pos;
layout(location = 1) in vec2 uv_left;
layout(location = 2) in vec2 uv_right;

layout(location = 0) out vec2 f_uv;

void main() {
	gl_Position = vec4(pos, 1.0);
	if(gl_ViewIndex == 0) {
		f_uv = uv_left;
	} else {
		f_uv = uv_right;
	}
}
