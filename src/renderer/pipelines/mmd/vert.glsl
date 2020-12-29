#version 450

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 0) out vec2 f_uv;
layout(location = 1) out vec3 f_normal;

layout(push_constant) uniform Mats {
	mat4 mvp;
} mats;

void main() {
	gl_Position = mats.mvp * vec4(pos, 1.0);
	
	f_uv = uv;
	
	mat3 normalMatrix = mat3(mats.mvp);
	normalMatrix = inverse(normalMatrix);
	normalMatrix = transpose(normalMatrix);
	f_normal = normalize(normal * normalMatrix);
}
