#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec3 pos_left;
layout(location = 1) in vec3 pos_right;
layout(location = 2) in vec4 color;

layout(location = 0) out vec4 f_color;

void main() {
	if(gl_ViewIndex == 0) {
		gl_Position = vec4(pos_left, 1.0);
	} else {
		gl_Position = vec4(pos_right, 1.0);
	}
	f_color = color;
}
