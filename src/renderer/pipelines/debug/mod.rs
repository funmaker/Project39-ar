use std::sync::Arc;
use vulkano::render_pass::RenderPass;
use vulkano::device::DeviceOwned;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::pipeline::graphics::color_blend::ColorBlendState;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::vertex_input::Vertex as VertexTy;
use vulkano::image::SampleCount;

mod vertex;

use super::{PipelineConstructor, PipelineError, pre_mul_alpha_blending};
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
				.color_blend_state(ColorBlendState::new(1).blend(pre_mul_alpha_blending()))
				.render_pass(render_pass.clone().first_subpass())
				.rasterization_state(RasterizationState::new().cull_mode(CullMode::Back))
				.multisample_state(MultisampleState {
					rasterization_samples: render_pass.clone().first_subpass().num_samples().unwrap_or(SampleCount::Sample1),
					..MultisampleState::new()
				})
				.build(device.clone())?
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
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>, PipelineError> {
		let device = render_pass.device();
		let vs = tex_vert::load(device.clone()).unwrap();
		let fs = tex_frag::load(device.clone()).unwrap();
		
		Ok(
			GraphicsPipeline::start()
				.vertex_input_state(TexturedVertex::per_vertex())
				.vertex_shader(vs.entry_point("main").unwrap(), ())
				.viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
				.fragment_shader(fs.entry_point("main").unwrap(), ())
				.color_blend_state(ColorBlendState::new(1).blend(pre_mul_alpha_blending()))
				.render_pass(render_pass.clone().first_subpass())
				.rasterization_state(RasterizationState::new().cull_mode(CullMode::Back))
				.multisample_state(MultisampleState {
					rasterization_samples: render_pass.clone().first_subpass().num_samples().unwrap_or(SampleCount::Sample1),
					..MultisampleState::new()
				})
				.build(device.clone())?
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
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>, PipelineError> {
		let device = render_pass.device();
		let vs = shape_vert::load(device.clone()).unwrap();
		let fs = shape_frag::load(device.clone()).unwrap();
		
		Ok(
			GraphicsPipeline::start()
				.vertex_input_state(DefaultPipelineVertex::per_vertex())
				.vertex_shader(vs.entry_point("main").unwrap(), ())
				.viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
				.fragment_shader(fs.entry_point("main").unwrap(), ())
				.depth_stencil_state(DepthStencilState::disabled())
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
