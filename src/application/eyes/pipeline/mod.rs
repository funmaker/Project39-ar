use std::sync::Arc;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::device::DeviceOwned;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::image::SampleCount;

mod vertex;

use crate::renderer::pipelines::{PipelineConstructor, PipelineError};
pub use vertex::Vertex;
pub use frag::ty::Pc;

mod vert {
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/application/eyes/pipeline/vert.glsl",
		spirv_version: "1.3",
		types_meta: { use bytemuck::{Zeroable, Pod}; #[derive(Clone, Copy, Zeroable, Pod)] }
	}
}

mod frag {
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/application/eyes/pipeline/frag.glsl",
		spirv_version: "1.3",
		types_meta: { use bytemuck::{Zeroable, Pod}; #[derive(Clone, Copy, Zeroable, Pod)] }
	}
}

pub struct BackgroundPipeline;

impl PipelineConstructor for BackgroundPipeline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>, PipelineError> {
		let device = render_pass.device();
		let vs = vert::load(device.clone()).unwrap();
		let fs = frag::load(device.clone()).unwrap();
		
		Ok(
			GraphicsPipeline::start()
				.vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
				.vertex_shader(vs.entry_point("main").unwrap(), ())
				.viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
				.fragment_shader(fs.entry_point("main").unwrap(), ())
				.depth_stencil_state(DepthStencilState::disabled())
				.rasterization_state(RasterizationState::new().cull_mode(CullMode::Back))
				.render_pass(render_pass.clone().first_subpass())
				.multisample_state(MultisampleState {
					rasterization_samples: render_pass.clone().first_subpass().num_samples().unwrap_or(SampleCount::Sample1),
					..MultisampleState::new()
				})
				.build(device.clone())?
		)
	}
}
