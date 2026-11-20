#version 450
#extension GL_EXT_multiview : require
#extension GL_GOOGLE_include_directive : require

layout(location = 0) in vec3 f_point;

layout(location = 0) out vec4 o_color;

#include "params.glsl"

layout(set = 0, binding = 1) uniform sampler2D tex;

#include "project.glsl"

void main() {
	// The mesh is the depth map of the left camera. Like SteamVR, texture it with the left camera in both eyes,
	// the right camera would show parts of the scene the mesh doesn't model (disocclusions, doubled edges).
	vec3 color;
	if (!sample_camera(f_point, 0, color)) {
		discard; // keep the dome underneath
	}
	o_color = vec4(color, 1.0);
}
