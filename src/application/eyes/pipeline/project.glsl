// Shared camera projection. Requires params.glsl and `tex`.

// Projects a tracking space point into the camera of `view` and samples it.
// Returns false if the point falls outside of that camera's part of the frame.
bool sample_camera(vec3 point, int view, out vec3 color) {
	vec3 cam = (params.tracking_to_camera[view] * vec4(point, 1.0)).xyz;
	cam = vec3(cam.x, -cam.y, -cam.z); // to image convention: x right, y down, z forward
	cam.x -= params.tan_shift[view].x * cam.z;

	// Fisheye
	vec4 k = params.coeffs[view];
	float r = length(cam.xy);
	float theta = atan(r, cam.z);
	float theta2 = theta * theta;
	float scale = theta * (1.0 + theta2 * (k.x + theta2 * (k.y + theta2 * (k.z + theta2 * k.w)))) / max(r, 1e-9);

	vec4 focal_center = params.focal_center[view];
	vec2 pixel = cam.xy * scale * focal_center.xy + focal_center.zw;

	vec4 bounds = params.bounds[view];
	if (any(lessThan(pixel, bounds.xy)) || any(greaterThan(pixel, bounds.zw))) {
		color = vec3(0.0);
		return false;
	}

	// Pixel centers are at integer coordinates
	color = texture(tex, (pixel + 0.5) / params.frame_size.xy).rgb;
	return true;
}
