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

void main() {
	vec2 focal = vec2(intrinsics.focal[gl_ViewIndex * 2], intrinsics.focal[gl_ViewIndex * 2 + 1]);
	vec4 proj = intrinsics.proj[gl_ViewIndex];
	vec4 coeffs = intrinsics.coeffs[gl_ViewIndex];
	vec2 scale = vec2(intrinsics.scale[gl_ViewIndex * 2], intrinsics.scale[gl_ViewIndex * 2 + 1]);
	vec2 center = vec2(intrinsics.center[gl_ViewIndex * 2], intrinsics.center[gl_ViewIndex * 2 + 1]);
	mat4 shift = pc.shift[gl_ViewIndex];
	
	vec4 dir = vec4(f_uv.x * proj[1] + proj[0], f_uv.y * proj[3] + proj[2], 1.0, 0.0);
	dir = shift * dir;
	
	float r = length(dir.xy);
	float theta = atan(r);
	
	theta *= 1.0 + coeffs.x * pow(theta, 2.0)
	             + coeffs.y * pow(theta, 4.0)
	             + coeffs.z * pow(theta, 6.0)
	             + coeffs.w * pow(theta, 8.0); // apply distortion
	
	vec2 uv = theta * dir.xy * focal / r; // to fish eye
	uv = uv * scale + center; // to image plane
	
	o_color = texture(tex, uv);
}
