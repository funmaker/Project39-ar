#version 450
#extension GL_EXT_multiview : require

layout(location = 0) in vec3 f_pos;
layout(location = 1) in vec3 f_viewPos;
layout(location = 2) in vec2 f_uv;
layout(location = 3) in vec3 f_normal;

layout(location = 0) out vec4 o_color;


layout(set = 0, binding = 0) uniform Commons {
	mat4 projection[2];
	mat4 view[2];
	vec4 light_direction[2];
	float ambient;
} commons;

layout(set = 0, binding = 1) uniform sampler2D tex;
layout(set = 0, binding = 2) uniform sampler2D normTex;

layout(push_constant) uniform Pc {
	mat4 model;
	vec4 color;
} pc;

void main() {
	// from: https://stackoverflow.com/a/44901073
	// derivations of the fragment position
	vec3 pos_dx = dFdx( f_pos );
	vec3 pos_dy = dFdy( f_pos );
	// derivations of the texture coordinate
	vec2 texC_dx = dFdx( f_uv );
	vec2 texC_dy = dFdy( f_uv );
	// tangent vector and binormal vector
	vec3 t = texC_dy.y * pos_dx - texC_dx.y * pos_dy;
	vec3 b = texC_dx.x * pos_dy - texC_dy.x * pos_dx;
	// orthonormalization
	t = cross( cross( f_normal, t ), t );
	b = cross( f_normal, t );
	mat3 tbn = mat3( normalize(t), normalize(b), f_normal );
	
	vec3 light_direction = commons.light_direction[gl_ViewIndex].xyz;
	
	vec3 tangent_normal = texture(normTex, f_uv).xyz;
	tangent_normal.z += 1.0 / 1.33;
	tangent_normal = normalize(tangent_normal * 2.0 - 1.0);
	vec3 world_normal = tbn * tangent_normal;
	
	vec3 color = texture(tex, f_uv).rgb;
	float reflection = dot(reflect(light_direction, world_normal), normalize(f_viewPos));
	
	vec3 ambient = color * 0.62;
	vec3 diffuse = color * clamp(dot(-world_normal, light_direction), 0.0, 1.0);
	vec3 specular = vec3(0.5377358, 0.5377358, 0.5377358) * pow(clamp(-reflection, 0.0, 1.0), 5.23);
	vec3 shadow = vec3(0.2830189, 0, 0.0488795) * pow(clamp(reflection, 0.0, 1.0), 3.21);
	
	o_color = vec4(ambient + diffuse + specular + shadow, 1);
}
