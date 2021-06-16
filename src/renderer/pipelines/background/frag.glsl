#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec2 f_uv;

layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 0) uniform Intrinsics {
	vec4 focal;
	vec4 proj[2];
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

void main() {
	vec4 proj = intrinsics.proj[gl_ViewIndex];
	mat4 shift = pc.shift[gl_ViewIndex];
	vec4 coeffs = intrinsics.coeffs[gl_ViewIndex];
	vec2 focal = split_vec4(intrinsics.focal);
	vec2 scale = split_vec4(intrinsics.scale);
	vec2 center = split_vec4(intrinsics.center);
	
	vec4 dir = vec4(f_uv.x * proj[1] + proj[0], f_uv.y * proj[3] + proj[2], 1.0, 0.0);
	dir = shift * dir;
	dir /= dir.z;
	
	float r = length(dir.xy);
	float theta = atan(r);
	
	theta *= 1.0 + coeffs.x * pow(theta, 2.0)
	             + coeffs.y * pow(theta, 4.0)
	             + coeffs.z * pow(theta, 6.0)
	             + coeffs.w * pow(theta, 8.0); // apply distortion
	
	vec2 uv = theta / r * dir.xy * focal; // to fish eye
	
	if(length(uv) < 0.004 && length(uv) >= 0.003) {
		o_color = vec4(1.0, 0.0, 1.0, 1.0);
		return;
	}
	
	uv = uv * scale + center; // to image plane
	
	vec2 tc = uv - vec2(0.25 + gl_ViewIndex * 0.5, 0.5);
	tc.x *= 2;
	if(length(tc) < 0.004 && length(tc) >= 0.003) {
		o_color = vec4(0.0, 1.0, 1.0, 1.0);
		return;
	}
	
	o_color = vec4(texture(tex, uv).rgb, 1.0);
}
