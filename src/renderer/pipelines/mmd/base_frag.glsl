#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec3 f_pos;
layout(location = 1) in vec2 f_uv;
layout(location = 2) in vec3 f_normal;

layout(location = 0) out vec4 o_color;


layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

layout(set = 1, binding = 0) uniform Material {
	vec4 color;
	vec3 specular;
	float specularity;
	vec3 ambient;
	uint sphere_mode;
} material;

layout(set = 1, binding = 1) uniform sampler2D tex;
layout(set = 1, binding = 2) uniform sampler2D toon;
layout(set = 1, binding = 3) uniform sampler2D sphere;

layout(push_constant) uniform Pc {
	mat4 model;
	vec4 _pad_color;
	float _pad_scale;
} pc;

layout(constant_id = 0) const bool transparent_pass = false;

void main() {
	vec3 light_direction = commons.light_direction[gl_ViewIndex].xyz;
	float lambert = dot(-f_normal, light_direction);
	
	vec3 reflected = normalize(-reflect(light_direction,f_normal));
	float spec_dot = max(0, dot(normalize(f_pos), reflected));
	float spec_weight = spec_dot == 0 ? 0.0f : pow( spec_dot, material.specularity );
	vec3 spec_light = material.specular * spec_weight;
	
	vec2 sphere_uv = f_normal.xy * 0.5 + 0.5;
	
	o_color = texture(tex, f_uv);
	
	if(material.sphere_mode == 1) {
		o_color.rgb *= texture(sphere, sphere_uv).rgb;
	} else if(material.sphere_mode == 2) {
		o_color.rgb += texture(sphere, sphere_uv).rgb;
	} else if(material.sphere_mode == 3) {
		// TODO: Implement additional vectors
	}
	
	o_color *= clamp(vec4(material.ambient, 0.0) + material.color, 0.0, 1.0);
	o_color.rgb += spec_light;
	o_color.rgb *= texture(toon, vec2(0.5, 0.5 - lambert * 0.5)).rgb;
	
	if(o_color.a < 1 && !transparent_pass) {
		discard;
	}
}
