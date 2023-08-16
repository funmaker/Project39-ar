pub use morph_comp::Pc as MorphPc;
pub use outline_vert::Pc;
use vulkano::pipeline::ComputePipeline;
use vulkano::pipeline::graphics::color_blend::ColorBlendState;

#[macro_use] mod macros;
mod vertex;

pub use vertex::Vertex;


mmd_shaders!(
	"vertex" base_vert = "src/component/model/mmd/pipeline/base_vert.glsl";
	"fragment" base_frag = "src/component/model/mmd/pipeline/base_frag.glsl";
	"vertex" outline_vert = "src/component/model/mmd/pipeline/outline_vert.glsl";
	"fragment" outline_frag = "src/component/model/mmd/pipeline/outline_frag.glsl";
	"compute" morph_comp = "src/component/model/mmd/pipeline/morph_comp.glsl";
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
			builder.rasterization_state(RasterizationState::new().cull_mode(CullMode::None))
		}
	}

	pub pipeline MMDPipelineTrans {
		shader vs = base_vert;
		shader fs = base_frag { transparent_pass: 1 };
		
		config builder {
			builder.color_blend_state(ColorBlendState::new(1).blend(pre_mul_alpha_blending()))
		}
	}
	
	pub pipeline MMDPipelineTransNoCull {
		shader vs = base_vert;
		shader fs = base_frag { transparent_pass: 1 };
		
		config builder {
			builder.color_blend_state(ColorBlendState::new(1).blend(pre_mul_alpha_blending()))
			       .rasterization_state(RasterizationState::new().cull_mode(CullMode::None))
		}
	}

	pub pipeline MMDPipelineOutline {
		shader vs = outline_vert;
		shader fs = outline_frag;
		
		config builder {
			builder.color_blend_state(ColorBlendState::new(1).blend(pre_mul_alpha_blending()))
			       .rasterization_state(RasterizationState::new().cull_mode(CullMode::Back))
		}
	}
);

pub const MORPH_GROUP_SIZE: usize = 32;

pub struct MMDPipelineMorphs;

impl PipelineConstructor for MMDPipelineMorphs {
	type PipeType = ComputePipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>, PipelineError> {
		let device = render_pass.device().clone();
		let cs = morph_comp::load(device.clone()).unwrap();
		
		Ok(ComputePipeline::new(device, cs.entry_point("main").unwrap(), &(), None, |_| {})?)
	}
}

