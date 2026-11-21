#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec3 f_proj;

layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 0) uniform Params {
	mat4 eye_to_tracking[2];
	mat4 tracking_to_camera[2];
	vec4 rawproj[2];
	vec4 coeffs[2];
	vec4 focal_center[2];
	vec4 bounds[2];
	vec2 tan_shift[2];
	vec4 dome;
	vec2 frame_size;
} params;

layout(set = 0, binding = 1) uniform sampler2D tex;

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
  
	vec3 point = origin + dir * t;
	vec3 cam = (params.tracking_to_camera[view] * vec4(point, 1.0)).xyz;
	cam = vec3(cam.x, -cam.y, -cam.z); // to image convention: x right, y down, z forward
	cam.xy -= params.tan_shift[view] * cam.z;
  
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
		o_color = vec4(0.0, 0.0, 0.0, 1.0);
		return;
	}
  
	// Pixel centers are at integer coordinates
	o_color = vec4(texture(tex, (pixel + 0.5) / params.frame_size.xy).rgb, 1.0);
}
