use std::sync::Arc;
use anyhow::Result;
use vulkano::device::DeviceOwned;
use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::RenderPass;

mod vertex;

use crate::renderer::pipelines::{build_graphics_pipeline, pre_mul_alpha_blending, simple_depth_test, PipelineConstructor};
pub use vertex::Vertex;
pub use vert::Pc;


mod vert {
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/component/toolgun/pipeline/vert.glsl",
		spirv_version: "1.3"
	}
}

mod frag {
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/component/toolgun/pipeline/frag.glsl",
		spirv_version: "1.3"
	}
}

pub struct ToolGunTextPipeline;

impl PipelineConstructor for ToolGunTextPipeline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();
		
		build_graphics_pipeline::<Vertex>(
			render_pass,
			vert::load(device.clone())?.entry_point("main").unwrap(),
			frag::load(device.clone())?.entry_point("main").unwrap(),
			simple_depth_test(),
			RasterizationState { cull_mode: CullMode::Back, ..RasterizationState::default() },
			Some(pre_mul_alpha_blending()),
		)
	}
}
