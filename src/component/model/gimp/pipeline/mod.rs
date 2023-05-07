use std::sync::Arc;
pub use vert::Pc;
use vulkano::device::DeviceOwned;
use vulkano::image::SampleCount;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::pipeline::graphics::color_blend::ColorBlendState;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState};
use vulkano::pipeline::graphics::vertex_input::Vertex as VertexTy;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::render_pass::RenderPass;

mod vertex;

use crate::renderer::pipelines::{PipelineConstructor, PipelineError, pre_mul_alpha_blending};
pub use vertex::Vertex;


mod vert {
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/component/model/gimp/pipeline/vert.glsl",
		spirv_version: "1.3"
	}
}

mod frag {
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/component/model/gimp/pipeline/frag.glsl",
		spirv_version: "1.3"
	}
}

pub struct GimpPipeline;

impl PipelineConstructor for GimpPipeline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>, PipelineError> {
		let device = render_pass.device();
		let vs = vert::load(device.clone()).unwrap();
		let fs = frag::load(device.clone()).unwrap();
		
		Ok(
			GraphicsPipeline::start()
				.vertex_input_state(Vertex::per_vertex())
				.vertex_shader(vs.entry_point("main").unwrap(), ())
				.viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
				.fragment_shader(fs.entry_point("main").unwrap(), ())
				.depth_stencil_state(DepthStencilState::simple_depth_test())
				.rasterization_state(RasterizationState::new().cull_mode(CullMode::Back))
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
