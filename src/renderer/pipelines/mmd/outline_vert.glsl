#version 450

layout(location = 0) in vec3 pos;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in float edge_scale;
layout(location = 4) in ivec4 bones_indices;
layout(location = 5) in vec4 bones_weights;

layout(location = 0) out vec2 f_uv;

layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

layout(set = 0, binding = 1) uniform Bones {
	mat4 mats[246];
} bones;

layout(set = 0, binding = 2) uniform ShapeKeys {
	vec4 offsets[110][35960];
} shapekeys;

layout(set = 0, binding = 3) uniform Morphs {
	ivec4 indexes[28];
	vec4 weights[28];
	int count;
} morphs;

layout(push_constant) uniform Pc {
	mat4 model;
	vec4 color;
	uint eye;
	float scale;
} pc;

void main() {
	mat4 mv = commons.view[pc.eye] * pc.model;
	mat4 mvp = commons.projection[pc.eye] * mv;
	mat3 normal_matrix = mat3(mv);
	
	mat4x3 anim = mat4x3(0);
	for(uint i = 0; i < 4; i++) {
		anim += mat4x3(bones.mats[bones_indices[i]]) * bones_weights[i];
	}
	
	vec3 morph_pos = vec3(0);
	for(uint i = 0; i < morphs.count; i++) {
		morph_pos += vec3(shapekeys.offsets[morphs.indexes[i / 4][i % 4]][gl_VertexIndex] * morphs.weights[i / 4][i % 4]);
	}
	
	vec4 view_pos = mv * vec4(anim * vec4(pos + morph_pos, 1.0), 1.0);
	vec3 view_normal = normalize(normal_matrix * (mat3(anim) * normal));
	view_pos += vec4(view_normal * pc.scale * edge_scale * length(vec3(view_pos)), 0.0);
	
	gl_Position = commons.projection[pc.eye] * view_pos;
	
	f_uv = uv;
}
