#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec3 f_proj;

layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 0) uniform Intrinsics {
	vec4 rawproj[2];
	vec4 focal;
	vec4 coeffs[2];
	vec4 scale;
	vec4 center;
} intrinsics;

layout(set = 0, binding = 1) uniform sampler2D tex;

layout(push_constant) uniform Pc {
	mat4 shift[2];
} pc;

vec2 split_vec4(vec4 vec) {
	return vec2(vec[gl_ViewIndex * 2], vec[gl_ViewIndex * 2 + 1]);
}

vec4 grid(vec2 dir) {
	vec4 ret = vec4(0.0, 0.0, 0.0, 1.0);
	float x = abs(dir.x);
	float y = abs(dir.y);
	
	ret.x = mod(x, 0.174533) * 100;
	ret.y = mod(y, 0.174533) * 100;
	ret.z = min(x, y) * 200;
	
	return ret;
}

void main() {
	mat4 shift = pc.shift[gl_ViewIndex];
	vec4 coeffs = intrinsics.coeffs[gl_ViewIndex];
	vec2 focal = split_vec4(intrinsics.focal);
	vec2 scale = split_vec4(intrinsics.scale);
	vec2 center = split_vec4(intrinsics.center);
	
	vec4 dir = vec4(f_proj, 0.0);
	
//	float hmd_grid = clamp(atan(min(abs(dir.x), abs(dir.y))) * 200, 0.0, 1.0);
	
	dir = shift * dir;
	dir /= -dir.z;
	
//	vec4 cam_grid = clamp(grid(dir.xy), vec4(0.0), vec4(1.0));

	float r = length(dir.xy);
	float theta = atan(r);

	theta *= 1.0 + coeffs.x * pow(theta, 2.0)
	             + coeffs.y * pow(theta, 4.0)
	             + coeffs.z * pow(theta, 6.0)
	             + coeffs.w * pow(theta, 8.0); // apply distortion

	vec2 uv = theta / r * dir.xy; // to fish eye
	uv *= focal;

//	if(length(uv) < 0.004 && length(uv) >= 0.003) {
//		o_color = vec4(1.0, 0.0, 1.0, 1.0);
//		return;
//	}
	
	uv.y *= -1.0;
	uv = uv * scale + center; // to image plane
	
	vec2 tc = uv - vec2(0.25 + gl_ViewIndex * 0.5, 0.5);
	tc.x *= 2;
	
//	if(length(tc) < 0.004 && length(tc) >= 0.003) {
//		o_color = vec4(0.0, 1.0, 1.0, 1.0);
//		return;
//	}
	
//	o_color = vec4(texture(tex, uv).rgb, 1.0) * cam_grid * vec4(hmd_grid, hmd_grid, hmd_grid, 1.0);
	o_color = vec4(texture(tex, uv).rgb, 1.0);
}
