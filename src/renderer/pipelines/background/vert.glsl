#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec2 pos;

layout(location = 0) out vec3 f_proj;

layout(set = 0, binding = 0) uniform Intrinsics {
	vec4 rawproj[2];
	vec4 focal;
	vec4 coeffs[2];
	vec4 scale;
	vec4 center;
} intrinsics;

void main() {
	gl_Position = vec4(pos, 0.0, 1.0);
	
	vec4 rawproj = intrinsics.rawproj[gl_ViewIndex];
	vec2 uv = pos * 0.5 + vec2(0.5);
	
	f_proj = vec3(
		-rawproj[0] + (rawproj[0] + rawproj[1]) * uv.x,
		 rawproj[2] - (rawproj[2] + rawproj[3]) * uv.y,
		-1.0
	);
}
