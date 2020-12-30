use std::sync::Arc;
use derive_deref::Deref;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::framebuffer::{RenderPassAbstract, Subpass};
use vulkano::pipeline::viewport::Viewport;
use vulkano::device::DeviceOwned;
use vulkano::pipeline::vertex::SingleBufferDefinition;
use vulkano::SafeDeref;
use vulkano::descriptor::PipelineLayoutAbstract;

use super::{Pipeline, PipelineError};
use crate::renderer::{model, RenderPass};

pub mod vert {
	#[allow(dead_code)]
	const SOURCE: &'static str = include_str!("./vert.glsl"); // https://github.com/vulkano-rs/vulkano/issues/1349
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/renderer/pipelines/mmd/vert.glsl"
	}
}

pub mod frag {
	#[allow(dead_code)]
	const SOURCE: &'static str = include_str!("./frag.glsl"); // https://github.com/vulkano-rs/vulkano/issues/1349
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/renderer/pipelines/mmd/frag.glsl"
	}
}

#[derive(Debug, Deref)]
pub struct MMDPipeline(
	GraphicsPipeline<
		SingleBufferDefinition<model::mmd::Vertex>,
		Box<dyn PipelineLayoutAbstract + Send + Sync>,
		Arc<dyn RenderPassAbstract + Send + Sync>
	>
);

unsafe impl SafeDeref for MMDPipeline {} // MMDPipeline is immutable, this should be safe

impl Pipeline for MMDPipeline {
	fn new(render_pass: &Arc<RenderPass>, frame_buffer_size: (u32, u32)) -> Result<Arc<dyn Pipeline>, PipelineError> {
		let device = render_pass.device();
		let vs = vert::Shader::load(device.clone()).unwrap();
		let fs = frag::Shader::load(device.clone()).unwrap();
		
		Ok(Arc::new(MMDPipeline(
			GraphicsPipeline::start()
				.vertex_input_single_buffer()
				.vertex_shader(vs.main_entry_point(), ())
				.viewports(Some(Viewport {
					origin: [0.0, 0.0],
					dimensions: [frame_buffer_size.0 as f32, frame_buffer_size.1 as f32],
					depth_range: 0.0..1.0,
				}))
				.fragment_shader(fs.main_entry_point(), ())
				.depth_stencil_simple_depth()
				.cull_mode_back()
				.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
				.build(device.clone())?
		)))
	}
}
