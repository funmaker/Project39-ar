use std::sync::Arc;
use anyhow::Result;
use vulkano::device::DeviceOwned;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::RenderPass;

mod vertex;

use super::{build_graphics_pipeline, pre_mul_alpha_blending, PipelineConstructor};
pub use vertex::{Vertex, TexturedVertex};
pub use shape_vert::Pc as ShapePc;


type DefaultPipelineVertex = super::default::Vertex;

mod vert {
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/renderer/pipelines/debug/vert.glsl",
		spirv_version: "1.3"
	}
}

mod frag {
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/renderer/pipelines/debug/frag.glsl",
		spirv_version: "1.3"
	}
}

pub struct DebugPipeline;

impl PipelineConstructor for DebugPipeline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();
		
		build_graphics_pipeline::<Vertex>(
			render_pass,
			vert::load(device.clone())?.entry_point("main").unwrap(),
			frag::load(device.clone())?.entry_point("main").unwrap(),
			DepthStencilState::default(),
			RasterizationState { cull_mode: CullMode::Back, ..RasterizationState::default() },
			Some(pre_mul_alpha_blending()),
		)
	}
}

mod tex_vert {
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/renderer/pipelines/debug/tex_vert.glsl",
		spirv_version: "1.3"
	}
}

mod tex_frag {
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/renderer/pipelines/debug/tex_frag.glsl",
		spirv_version: "1.3"
	}
}

pub struct DebugTexturedPipeline;

impl PipelineConstructor for DebugTexturedPipeline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();
		
		build_graphics_pipeline::<TexturedVertex>(
			render_pass,
			tex_vert::load(device.clone())?.entry_point("main").unwrap(),
			tex_frag::load(device.clone())?.entry_point("main").unwrap(),
			DepthStencilState::default(),
			RasterizationState { cull_mode: CullMode::Back, ..RasterizationState::default() },
			Some(pre_mul_alpha_blending()),
		)
	}
}


mod shape_vert {
vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/renderer/pipelines/debug/shape_vert.glsl",
		spirv_version: "1.3"
	}
}

mod shape_frag {
vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/renderer/pipelines/debug/shape_frag.glsl",
		spirv_version: "1.3"
	}
}

pub struct DebugShapePipeline;

impl PipelineConstructor for DebugShapePipeline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();
		
		build_graphics_pipeline::<DefaultPipelineVertex>(
			render_pass,
			shape_vert::load(device.clone())?.entry_point("main").unwrap(),
			shape_frag::load(device.clone())?.entry_point("main").unwrap(),
			DepthStencilState::default(),
			RasterizationState { cull_mode: CullMode::Back, ..RasterizationState::default() },
			Some(pre_mul_alpha_blending()),
		)
	}
}
