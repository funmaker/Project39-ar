use std::sync::Arc;
use anyhow::Result;
use vulkano::device::DeviceOwned;
use vulkano::image::SampleCount;
use vulkano::pipeline::{GraphicsPipeline, StateMode};
use vulkano::pipeline::graphics::color_blend::ColorBlendState;
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState};
use vulkano::pipeline::graphics::vertex_input::Vertex as VertexTy;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::render_pass::RenderPass;

mod vertex;

use crate::renderer::pipelines::{pre_mul_alpha_blending, PipelineConstructor};
pub use vertex::Vertex;
pub use vert::Pc;


mod vert {
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/component/srgb_test/pipeline/vert.glsl",
		spirv_version: "1.3"
	}
}

mod frag {
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/component/srgb_test/pipeline/frag.glsl",
		spirv_version: "1.3"
	}
}

pub struct SrgbTestPipeline;

impl PipelineConstructor for SrgbTestPipeline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();
		let vs = vert::load(device.clone()).unwrap();
		let fs = frag::load(device.clone()).unwrap();
		
		Ok(
			GraphicsPipeline::start()
				.vertex_input_state(Vertex::per_vertex())
				.vertex_shader(vs.entry_point("main").unwrap(), ())
				.viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
				.fragment_shader(fs.entry_point("main").unwrap(), ())
				.depth_stencil_state(DepthStencilState {
					depth: Some(DepthState {
						enable_dynamic: false,
						compare_op: StateMode::Fixed(CompareOp::Always),
						write_enable: StateMode::Fixed(true),
					}),
					depth_bounds: None,
					stencil: None,
				})
				.rasterization_state(RasterizationState::new().cull_mode(CullMode::None))
				.color_blend_state(ColorBlendState::new(1).blend(pre_mul_alpha_blending()))
				.render_pass(render_pass.clone().first_subpass())
				.multisample_state(MultisampleState {
					rasterization_samples: render_pass.clone().first_subpass().num_samples().unwrap_or(SampleCount::Sample1),
					..MultisampleState::new()
				})
				.build(device.clone())?
		)
	}
}
