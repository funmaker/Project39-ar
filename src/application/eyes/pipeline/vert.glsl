#version 450
#extension GL_EXT_multiview : require
#extension GL_GOOGLE_include_directive : require

layout(location = 0) in vec2 pos;

layout(location = 0) out vec3 f_proj;

#include "params.glsl"

void main() {
	gl_Position = vec4(pos, 0.0, 1.0);

	vec4 rawproj = params.rawproj[gl_ViewIndex];
	vec2 uv = pos * 0.5 + vec2(0.5);

	// rawproj is (-left, right, -top, bottom) of OpenVR's GetProjectionRaw. Its "top"/"bottom" are in y-down
	// convention: `bottom` is the upper edge (see ComposeProjection), uv.y = 0 is the top of the screen.
	f_proj = vec3(
		-rawproj[0] + (rawproj[0] + rawproj[1]) * uv.x,
		 rawproj[3] - (rawproj[2] + rawproj[3]) * uv.y,
		-1.0
	);
}
