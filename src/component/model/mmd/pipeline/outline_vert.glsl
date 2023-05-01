#version 450
#extension GL_EXT_multiview : require
#include <commons.glsl>

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in float edge_scale;
layout(location = 4) in uvec4 bones_indices;
layout(location = 5) in vec4 bones_weights;
layout(location = 6) in vec3 sdef_c;
layout(location = 7) in vec3 sdef_r0;
layout(location = 8) in vec3 sdef_r1;

layout(location = 0) out vec2 f_uv;

layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

layout(set = 0, binding = 1) readonly buffer Bones {
	mat4 mats[];
} bones;

layout(set = 0, binding = 2) readonly buffer Offsets {
	ivec4 vecs[];
} offsets;

layout(push_constant) uniform Pc {
	mat4 model;
	vec4 color;
	float scale;
} pc;

void main() {
	mat4 mv = commons.view[gl_ViewIndex] * pc.model;
	mat4 mvp = commons.projection[gl_ViewIndex] * mv;
	mat3 normal_matrix = mat3(mv);
	
	VertDesc vert = VertDesc(pos, normal);
	
	vec3 morph_pos = vec3(offsets.vecs[gl_VertexIndex].xyz) / 1000000.0;
	vert.pos += morph_pos;
	
	if(dot(sdef_c, sdef_c) > 0.01) {
		SDEFBone bone0 = GET_SDEF_BONE(0);
		SDEFBone bone1 = GET_SDEF_BONE(1);
		SDEFParams sdef = SDEFParams(sdef_c, sdef_r0, sdef_r1);
		
		vert = blend_sdef(vert, bone0, bone1, sdef);
	} else {
		BDEFBone bones[4] = BDEFBone[](
			GET_BDEF_BONE(0),
			GET_BDEF_BONE(1),
			GET_BDEF_BONE(2),
			GET_BDEF_BONE(3)
		);
		
		vert = blend_bdef(vert, bones);
	}
	
	vec4 view_pos = mv * vec4(vert.pos, 1.0);
	vec3 view_normal = normalize(normal_matrix * vert.norm);
	view_pos += vec4(view_normal * pc.scale * edge_scale * length(vec3(view_pos)), 0.0);
	
	gl_Position = commons.projection[gl_ViewIndex] * view_pos;
	f_uv = uv;
}
