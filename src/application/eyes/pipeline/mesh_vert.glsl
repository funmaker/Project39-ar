#version 450
#extension GL_EXT_multiview : require
#extension GL_GOOGLE_include_directive : require

// 3D passthrough: SteamVR's depth mesh (raw tracking space) drawn in place of the dome.

layout(location = 0) in vec3 pos;

layout(location = 0) out vec3 f_point;

#include "params.glsl"

void main() {
	int view = gl_ViewIndex;
	vec3 point = (params.raw_to_tracking * vec4(pos, 1.0)).xyz;
	vec3 eye = (params.tracking_to_eye[view] * vec4(point, 1.0)).xyz;

	// Same frustum as vert.glsl: rawproj is (-left, right, -top, bottom), "bottom" being the upper edge.
	vec4 rawproj = params.rawproj[view];
	float w = -eye.z;
	vec2 tangent = eye.xy / w;
	vec2 ndc = vec2(
		(tangent.x + rawproj[0]) / (rawproj[0] + rawproj[1]),
		(rawproj[3] - tangent.y) / (rawproj[2] + rawproj[3])
	) * 2.0 - 1.0;

	// Constant depth at half the clip range, points behind the eye (w < 0) get clipped.
	gl_Position = vec4(ndc * w, 0.5 * w, w);
	f_point = point;
}
