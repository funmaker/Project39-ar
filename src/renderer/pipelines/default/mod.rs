use std::sync::Arc;
use anyhow::Result;
use vulkano::device::DeviceOwned;
use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::RenderPass;

mod vertex;

use super::{build_graphics_pipeline, pre_mul_alpha_blending, simple_depth_test, PipelineConstructor};
pub use vertex::Vertex;
pub use glow_vert::Pc as GlowPc;
pub use vert::Pc;


mod vert {
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/renderer/pipelines/default/vert.glsl",
		spirv_version: "1.3"
	}
}

mod frag {
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/renderer/pipelines/default/frag.glsl",
		spirv_version: "1.3"
	}
}

mod glow_vert {
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/renderer/pipelines/default/glow_vert.glsl",
		spirv_version: "1.3"
	}
}

mod glow_frag {
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/renderer/pipelines/default/glow_frag.glsl",
		spirv_version: "1.3"
	}
}

pub struct DefaultPipeline;

impl PipelineConstructor for DefaultPipeline {
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

pub struct DefaultGlowPipeline;

impl PipelineConstructor for DefaultGlowPipeline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();
		
		build_graphics_pipeline::<Vertex>(
			render_pass,
			glow_vert::load(device.clone())?.entry_point("main").unwrap(),
			glow_frag::load(device.clone())?.entry_point("main").unwrap(),
			simple_depth_test(),
			RasterizationState { cull_mode: CullMode::Back, ..RasterizationState::default() },
			Some(pre_mul_alpha_blending()),
		)
	}
}
