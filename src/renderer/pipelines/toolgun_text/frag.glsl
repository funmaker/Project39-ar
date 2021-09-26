#version 450

layout(location = 0) in vec2 f_uv;

layout(location = 0) out vec4 o_color;

layout(set = 1, binding = 0) uniform sampler2D tex;

void main() {
	if(texture(tex, f_uv).r <= 0) discard;
	
	o_color = vec4(1.0) * texture(tex, f_uv).r;
}
