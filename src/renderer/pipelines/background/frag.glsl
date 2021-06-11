#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec2 f_uv;

layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 0) uniform Intrinsics {
	vec4 hfov;
	vec4 dfov;
	vec4 coeffs[2];
	vec4 scale;
	vec4 center;
} intrinsics;

layout(set = 0, binding = 1) uniform sampler2D tex;

//layout(push_constant) uniform Pc {
//	mat4 model;
//} pc;

void main() {
	vec2 uv = f_uv;
	vec2 hfov = vec2(intrinsics.hfov[gl_ViewIndex * 2], intrinsics.hfov[gl_ViewIndex * 2 + 1]);
	vec2 dfov = vec2(intrinsics.dfov[gl_ViewIndex * 2], intrinsics.dfov[gl_ViewIndex * 2 + 1]);
	vec4 coeffs = intrinsics.coeffs[gl_ViewIndex];
	vec2 scale = vec2(intrinsics.scale[gl_ViewIndex * 2], intrinsics.scale[gl_ViewIndex * 2 + 1]);
	vec2 center = vec2(intrinsics.center[gl_ViewIndex * 2], intrinsics.center[gl_ViewIndex * 2 + 1]);
	
	uv = uv * 2.0 - 1.0; // normalize
	uv *= tan(dfov); // to image plane
	
	float r = length(uv);
	float theta = atan(r);
	
	theta *= 1.0 + coeffs.x * pow(theta, 2.0)
	             + coeffs.y * pow(theta, 4.0)
	             + coeffs.z * pow(theta, 6.0)
	             + coeffs.w * pow(theta, 8.0); // apply distortion
	
	uv *= theta / hfov / r; // to fish eye
	
	uv = uv / 2.0 * scale + center; // to texture
	
	o_color = texture(tex, uv);
}
