use std::sync::Arc;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::device::DeviceOwned;
use vulkano::pipeline::graphics::color_blend::ColorBlendState;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState};
use vulkano::pipeline::graphics::viewport::ViewportState;

mod vertex;

use super::{PipelineConstructor, PipelineError, pre_mul_alpha_blending};
pub use vertex::{Vertex, TexturedVertex};

type DefaultPipelineVertex = super::default::Vertex;

mod vert {
	#[allow(dead_code)]
	const SOURCE: &'static str = include_str!("./vert.glsl"); // https://github.com/vulkano-rs/vulkano/issues/1349
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/renderer/pipelines/debug/vert.glsl",
		spirv_version: "1.3"
	}
}

mod frag {
	#[allow(dead_code)]
	const SOURCE: &'static str = include_str!("./frag.glsl"); // https://github.com/vulkano-rs/vulkano/issues/1349
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/renderer/pipelines/debug/frag.glsl",
		spirv_version: "1.3"
	}
}

pub struct DebugPipeline;

impl PipelineConstructor for DebugPipeline {
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
				.color_blend_state(ColorBlendState::new(1).blend(pre_mul_alpha_blending()))
				.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
				.rasterization_state(RasterizationState::new().cull_mode(CullMode::Back))
				.build(device.clone())?
		)
	}
}

mod tex_vert {
	#[allow(dead_code)]
	const SOURCE: &'static str = include_str!("./tex_vert.glsl"); // https://github.com/vulkano-rs/vulkano/issues/1349
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/renderer/pipelines/debug/tex_vert.glsl",
		spirv_version: "1.3"
	}
}

mod tex_frag {
	#[allow(dead_code)]
	const SOURCE: &'static str = include_str!("./tex_frag.glsl"); // https://github.com/vulkano-rs/vulkano/issues/1349
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/renderer/pipelines/debug/tex_frag.glsl",
		spirv_version: "1.3"
	}
}

pub struct DebugTexturedPipeline;

impl PipelineConstructor for DebugTexturedPipeline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>, PipelineError> {
		let device = render_pass.device();
		let vs = tex_vert::load(device.clone()).unwrap();
		let fs = tex_frag::load(device.clone()).unwrap();
		
		Ok(
			GraphicsPipeline::start()
				.vertex_input_state(BuffersDefinition::new().vertex::<TexturedVertex>())
				.vertex_shader(vs.entry_point("main").unwrap(), ())
				.viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
				.fragment_shader(fs.entry_point("main").unwrap(), ())
				.color_blend_state(ColorBlendState::new(1).blend(pre_mul_alpha_blending()))
				.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
				.rasterization_state(RasterizationState::new().cull_mode(CullMode::Back))
				.build(device.clone())?
		)
	}
}


mod shape_vert {
	#[allow(dead_code)]
	const SOURCE: &'static str = include_str!("./shape_vert.glsl"); // https://github.com/vulkano-rs/vulkano/issues/1349
vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/renderer/pipelines/debug/shape_vert.glsl",
		spirv_version: "1.3"
	}
}

mod shape_frag {
	#[allow(dead_code)]
	const SOURCE: &'static str = include_str!("./shape_frag.glsl"); // https://github.com/vulkano-rs/vulkano/issues/1349
vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/renderer/pipelines/debug/shape_frag.glsl",
		spirv_version: "1.3"
	}
}

pub struct DebugShapePipeline;

impl PipelineConstructor for DebugShapePipeline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>, PipelineError> {
		let device = render_pass.device();
		let vs = shape_vert::load(device.clone()).unwrap();
		let fs = shape_frag::load(device.clone()).unwrap();
		
		Ok(
			GraphicsPipeline::start()
				.vertex_input_state(BuffersDefinition::new().vertex::<DefaultPipelineVertex>())
				.vertex_shader(vs.entry_point("main").unwrap(), ())
				.viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
				.fragment_shader(fs.entry_point("main").unwrap(), ())
				.depth_stencil_state(DepthStencilState::disabled())
				.rasterization_state(RasterizationState::new().cull_mode(CullMode::Back))
				.color_blend_state(ColorBlendState::new(1).blend(pre_mul_alpha_blending()))
				.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
				.build(device.clone())?
		)
	}
}
