#version 450
#extension GL_EXT_multiview : require
#extension GL_GOOGLE_include_directive : require

// Passthrough reprojection matching SteamVR's room view (tracked_camera_reprojection_vs/ps):
// eye rays are intersected with a dome standing on the floor (and the floor itself), the hit point is
// projected into the camera from its real position at the frame capture time, then through the fisheye lens model.

layout(location = 0) in vec3 f_proj;

layout(location = 0) out vec4 o_color;

#include "params.glsl"

layout(set = 0, binding = 1) uniform sampler2D tex;

#include "project.glsl"

void main() {
	int view = gl_ViewIndex;
	mat4 eye_to_tracking = params.eye_to_tracking[view];

	vec3 origin = eye_to_tracking[3].xyz;
	vec3 dir = normalize(mat3(eye_to_tracking) * f_proj);

	// Dome (sphere around the base center)
	vec3 oc = origin - params.dome.xyz;
	float b = dot(dir, oc);
	float c = dot(oc, oc) - params.dome.w * params.dome.w;
	float t = -b + sqrt(max(b * b - c, 0.0));

	// Floor
	float height = origin.y - params.dome.y;
	if (dir.y < 0.0 && height > 0.0) {
		t = min(t, height / -dir.y);
	}

	// Pixels outside of the camera's part of the frame stay black (like SteamVR)
	vec3 color;
	sample_camera(origin + dir * t, view, color);
	o_color = vec4(color, 1.0);
}
