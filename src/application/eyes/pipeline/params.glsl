// Shared by vert.glsl and frag.glsl. Updated every frame.
// All positions are in tracking space (OpenVR standing space in VR, world space in novr), y up.
layout(set = 0, binding = 0) uniform Params {
	// Eye space -> tracking space at render time.
	mat4 eye_to_tracking[2];
	// Tracking space -> camera space (OpenVR convention: x right, y up, -z forward) at camera frame capture time.
	mat4 tracking_to_camera[2];
	// Tangents of the eye's half fov: left, right, top, bottom.
	vec4 rawproj[2];
	// Fisheye distortion coefficients.
	vec4 coeffs[2];
	// Focal length (xy) and principal point (zw) in camera frame pixels.
	vec4 focal_center[2];
	// Usable part of the camera frame (pixels): min xy, max zw.
	vec4 bounds[2];
	// Projection dome: center of its base on the floor (xyz) and radius (w).
	vec4 dome;
	// Camera frame size (xy).
	vec4 frame_size;
} params;
