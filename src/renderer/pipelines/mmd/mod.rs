#[macro_use] mod macros;

mmd_shaders!(
	"vertex" base_vert = "src/renderer/pipelines/mmd/base_vert.glsl";
	"fragment" base_frag = "src/renderer/pipelines/mmd/base_frag.glsl";
	"vertex" outline_vert = "src/renderer/pipelines/mmd/outline_vert.glsl";
	"fragment" outline_frag = "src/renderer/pipelines/mmd/outline_frag.glsl";
);

mmd_pipelines!(
	pub pipeline MMDPipelineOpaque {
		shader vs = base_vert;
		shader fs = base_frag { transparent_pass: 0 };
		
		config builder {
			builder
		}
	}
	
	pub pipeline MMDPipelineOpaqueNoCull {
		shader vs = base_vert;
		shader fs = base_frag { transparent_pass: 0 };
		
		config builder {
			builder.cull_mode_disabled()
		}
	}

	pub pipeline MMDPipelineTrans {
		shader vs = base_vert;
		shader fs = base_frag { transparent_pass: 1 };
		
		config builder {
			builder.blend_collective(pre_mul_alpha_blending())
		}
	}
	
	pub pipeline MMDPipelineTransNoCull {
		shader vs = base_vert;
		shader fs = base_frag { transparent_pass: 1 };
		
		config builder {
			builder.blend_collective(pre_mul_alpha_blending())
			       .cull_mode_disabled()
		}
	}

	pub pipeline MMDPipelineOutline {
		shader vs = outline_vert;
		shader fs = outline_frag;
		
		config builder {
			builder.blend_collective(pre_mul_alpha_blending())
			       .cull_mode_front()
		}
	}
);

