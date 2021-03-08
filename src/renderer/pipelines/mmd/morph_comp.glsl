#version 450

layout(local_size_x = 32, local_size_y = 1, local_size_z = 1) in;

layout(set = 0, binding = 0) buffer Morphs {
	ivec4 morphs[];
} morphs;

layout(set = 0, binding = 1) buffer MorphsDesc {
	ivec4 offsets[];
} morphsDesc;

layout(set = 0, binding = 2) buffer Offsets {
	ivec4 offsets[];
} outBuf;

layout(push_constant) uniform Pc {
	uint morphsMaxSize;
} pc;

void main() {
	uint oid = gl_GlobalInvocationID.x;
	uint mid = gl_GlobalInvocationID.y;
	uint mmid = morphs.morphs[mid / 2][(mid % 2) * 2];
	float scale = intBitsToFloat(morphs.morphs[mid / 2][(mid % 2) * 2 + 1]);
	
	ivec3 offset = ivec3(morphsDesc.offsets[mmid * pc.morphsMaxSize + oid].xyz * scale);
	int vertex = morphsDesc.offsets[mmid * pc.morphsMaxSize + oid].w;
	
	atomicAdd(outBuf.offsets[vertex].x, offset.x);
	atomicAdd(outBuf.offsets[vertex].y, offset.y);
	atomicAdd(outBuf.offsets[vertex].z, offset.z);
}
