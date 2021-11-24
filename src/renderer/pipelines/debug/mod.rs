use std::sync::Arc;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::pipeline::viewport::Viewport;
use vulkano::device::DeviceOwned;

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
	
	fn new(render_pass: &Arc<RenderPass>, frame_buffer_size: (u32, u32)) -> Result<Arc<Self::PipeType>, PipelineError> {
		let device = render_pass.device();
		let vs = vert::Shader::load(device.clone()).unwrap();
		let fs = frag::Shader::load(device.clone()).unwrap();
		
		Ok(Arc::new(
			GraphicsPipeline::start()
				.vertex_input_single_buffer::<Vertex>()
				.vertex_shader(vs.main_entry_point(), ())
				.viewports(Some(Viewport {
					origin: [0.0, 0.0],
					dimensions: [frame_buffer_size.0 as f32, frame_buffer_size.1 as f32],
					depth_range: 0.0..1.0,
				}))
				.fragment_shader(fs.main_entry_point(), ())
				.blend_collective(pre_mul_alpha_blending())
				.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
				.cull_mode_back()
				.build(device.clone())?
		))
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
	
	fn new(render_pass: &Arc<RenderPass>, frame_buffer_size: (u32, u32)) -> Result<Arc<Self::PipeType>, PipelineError> {
		let device = render_pass.device();
		let vs = tex_vert::Shader::load(device.clone()).unwrap();
		let fs = tex_frag::Shader::load(device.clone()).unwrap();
		
		Ok(Arc::new(
			GraphicsPipeline::start()
				.vertex_input_single_buffer::<TexturedVertex>()
				.vertex_shader(vs.main_entry_point(), ())
				.viewports(Some(Viewport {
					origin: [0.0, 0.0],
					dimensions: [frame_buffer_size.0 as f32, frame_buffer_size.1 as f32],
					depth_range: 0.0..1.0,
				}))
				.fragment_shader(fs.main_entry_point(), ())
				.blend_collective(pre_mul_alpha_blending())
				.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
				.cull_mode_back()
				.build(device.clone())?
		))
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
	
	fn new(render_pass: &Arc<RenderPass>, frame_buffer_size: (u32, u32)) -> Result<Arc<Self::PipeType>, PipelineError> {
		let device = render_pass.device();
		let vs = shape_vert::Shader::load(device.clone()).unwrap();
		let fs = shape_frag::Shader::load(device.clone()).unwrap();
		
		Ok(Arc::new(
			GraphicsPipeline::start()
				.vertex_input_single_buffer::<DefaultPipelineVertex>()
				.vertex_shader(vs.main_entry_point(), ())
				.viewports(Some(Viewport {
					origin: [0.0, 0.0],
					dimensions: [frame_buffer_size.0 as f32, frame_buffer_size.1 as f32],
					depth_range: 0.0..1.0,
				}))
				.fragment_shader(fs.main_entry_point(), ())
				.depth_stencil_simple_depth()
				.cull_mode_back()
				.blend_collective(pre_mul_alpha_blending())
				.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
				.build(device.clone())?
		))
	}
}
