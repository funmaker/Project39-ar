use std::sync::Arc;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState};
use vulkano::pipeline::graphics::color_blend::ColorBlendState;
use vulkano::device::DeviceOwned;

mod vertex;

use crate::renderer::pipelines::pre_mul_alpha_blending;
use crate::renderer::pipelines::{PipelineConstructor, PipelineError};
pub use vertex::Vertex;

mod vert {
	#[allow(dead_code)]
	const SOURCE: &'static str = include_str!("vert.glsl"); // https://github.com/vulkano-rs/vulkano/issues/1349
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/component/toolgun/pipeline/vert.glsl",
		spirv_version: "1.3"
	}
}

mod frag {
	#[allow(dead_code)]
	const SOURCE: &'static str = include_str!("frag.glsl"); // https://github.com/vulkano-rs/vulkano/issues/1349
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/component/toolgun/pipeline/frag.glsl",
		spirv_version: "1.3"
	}
}

pub struct ToolGunTextPipeline;

impl PipelineConstructor for ToolGunTextPipeline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>, frame_buffer_size: (u32, u32)) -> Result<Arc<Self::PipeType>, PipelineError> {
		let device = render_pass.device();
		let vs = vert::load(device.clone()).unwrap();
		let fs = frag::load(device.clone()).unwrap();
		
		Ok(
			GraphicsPipeline::start()
				.vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
				.vertex_shader(vs.entry_point("main").unwrap(), ())
				.viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([
					Viewport {
						origin: [0.0, 0.0],
						dimensions: [frame_buffer_size.0 as f32, frame_buffer_size.1 as f32],
						depth_range: 0.0..1.0,
					},
				]))
				.fragment_shader(fs.entry_point("main").unwrap(), ())
				.depth_stencil_state(DepthStencilState::simple_depth_test())
				.rasterization_state(RasterizationState::new().cull_mode(CullMode::Back))
				.color_blend_state(ColorBlendState::new(1).blend(pre_mul_alpha_blending()))
				.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
				.build(device.clone())?
		)
	}
}
