use vulkano::pipeline::ComputePipeline;
use vulkano::descriptor::pipeline_layout::PipelineLayout;

#[macro_use] mod macros;
mod vertex;

pub use vertex::Vertex;

mmd_shaders!(
	"vertex" base_vert = "src/renderer/pipelines/mmd/base_vert.glsl";
	"fragment" base_frag = "src/renderer/pipelines/mmd/base_frag.glsl";
	"vertex" outline_vert = "src/renderer/pipelines/mmd/outline_vert.glsl";
	"fragment" outline_frag = "src/renderer/pipelines/mmd/outline_frag.glsl";
	"compute" morph_comp = "src/renderer/pipelines/mmd/morph_comp.glsl";
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

pub const MORPH_GROUP_SIZE: usize = 32;

#[derive(Debug, Deref)]
pub struct MMDPipelineMorphs(ComputePipeline<PipelineLayout<morph_comp::Layout>>);

unsafe impl SafeDeref for MMDPipelineMorphs {}

impl Pipeline for MMDPipelineMorphs {
	fn new(render_pass: &Arc<RenderPass>, _frame_buffer_size: (u32, u32)) -> Result<Arc<dyn Pipeline>, PipelineError> {
		let device = render_pass.device().clone();
		let cs = morph_comp::Shader::load(device.clone()).unwrap();
		
		Ok(Arc::new(MMDPipelineMorphs(
			ComputePipeline::new(device, &cs.main_entry_point(), &(), None)?
		)))
	}
}

