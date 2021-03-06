#version 450

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in float edge_scale;
layout(location = 4) in uvec4 bones_indices;
layout(location = 5) in vec4 bones_weights;

layout(location = 0) out vec3 f_pos;
layout(location = 1) out vec2 f_uv;
layout(location = 2) out vec3 f_normal;

layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

layout(set = 0, binding = 1) uniform Bones {
	mat4 mats[246];
} bones;

layout(set = 0, binding = 2) uniform Offsets {
	ivec4 vecs[35960];
} offsets;

layout(push_constant) uniform Pc {
	mat4 model;
	uint eye;
} pc;

void main() {
	mat4 mv = commons.view[pc.eye] * pc.model;
	mat4 mvp = commons.projection[pc.eye] * mv;
	mat3 normal_matrix = mat3(mv);
	
	vec4 anim_pos = vec4(0);
	vec3 anim_normal = vec3(0);
	
	mat4x3 anim = mat4x3(0);
	for(uint i = 0; i < 4; i++) {
		anim += mat4x3(bones.mats[bones_indices[i]]) * bones_weights[i];
	}
	
	vec3 morph_offset = vec3(offsets.vecs[gl_VertexIndex].xyz) / 1000000.0;
	
	gl_Position = mvp * vec4(anim * vec4(pos + morph_pos, 1.0), 1.0);
	
	f_pos = vec3(mv * vec4(pos, 1.0));
	f_uv = uv;
	f_normal = normalize(normal_matrix * (mat3(anim) * anim_normal));
}
