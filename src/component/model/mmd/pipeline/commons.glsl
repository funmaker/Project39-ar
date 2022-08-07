
// Based on https://qiita.com/dragonmeteor/items/0211166d55bb2eb7c07c

struct VertDesc {
	vec3 pos;
	vec3 norm;
};

vec4 q_blend(vec4 q1, float w1, vec4 q2, float w2) {
	if(dot(q1, q2) < 0) {
		q2 = -q2;
	}
	
	return normalize(w1*q1 + w2*q2);
}

mat3 q_mat(vec4 q) {
	float i = q[0];
	float j = q[1];
	float k = q[2];
	float w = q[3];
	
	float ww = w * w;
	float ii = i * i;
	float jj = j * j;
	float kk = k * k;
	float ij = i * j * 2.0;
	float wk = w * k * 2.0;
	float wj = w * j * 2.0;
	float ik = i * k * 2.0;
	float jk = j * k * 2.0;
	float wi = w * i * 2.0;
	
	return mat3(
		ww + ii - jj - kk, ij - wk,           wj + ik,
		wk + ij,           ww - ii + jj - kk, jk - wi,
		ik - wj,           wi + jk,           ww - ii - jj + kk
	);
}

struct BDEFBone {
	float weight;
	mat4x3 mat;
};

#define GET_BDEF_BONE(id)                    \
	BDEFBone(                                 \
		bones_weights[id],                    \
		mat4x3(bones.mats[bones_indices[id]]) \
	)

VertDesc blend_bdef(VertDesc desc, BDEFBone bones[4]) {
	mat4x3 anim = mat4x3(0);
	
	for(uint i = 0; i < 4; i++) {
		anim += mat4x3(bones[i].mat) * bones[i].weight;
	}
	
	vec3 pos = anim * vec4(desc.pos, 1.0);
	vec3 norm = (mat3(anim) * desc.norm);
	
	return VertDesc(pos, norm);
}

struct SDEFBone {
	float weight;
	mat4x3 mat;
	vec4 rot;
};

#define GET_SDEF_BONE(id)                        \
	SDEFBone(                                    \
		bones_weights[id],                       \
		mat4x3(bones.mats[bones_indices[id]]),   \
		vec4(                                    \
			bones.mats[bones_indices[id]][0][3], \
			bones.mats[bones_indices[id]][1][3], \
			bones.mats[bones_indices[id]][2][3], \
			bones.mats[bones_indices[id]][3][3]  \
		)                                        \
	)

struct SDEFParams {
	vec3 c;
	vec3 r0;
	vec3 r1;
};

VertDesc blend_sdef(VertDesc vert, SDEFBone bone0, SDEFBone bone1, SDEFParams sdef) {
	float w0 = bone0.weight;
	mat4x3 mat0 = bone0.mat;
	vec4 rot0 = bone0.rot;
	
	float w1 = bone1.weight;
	mat4x3 mat1 = bone1.mat;
	vec4 rot1 = bone1.rot;
	
	mat4x3 bmat = w0 * mat0 + w1 * mat1;
	vec4 brot = q_blend(rot0, w0, rot1, w1);
	mat3 brotmat = q_mat(brot);
	
	vec3 cpos = vert.pos - sdef.c;
	
	vec3 pos = bmat * vec4(sdef.c, 1)
	         + brotmat * cpos
	         + w0 * w1 * (mat0 - mat1) * vec4(sdef.r0 - sdef.r1, 0) / 2;
	vec3 norm = brotmat * vert.norm;
	
	return VertDesc(pos, norm);
}
