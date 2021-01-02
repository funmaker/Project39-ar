use std::sync::Arc;
use derive_deref::Deref;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::framebuffer::{RenderPassAbstract, Subpass};
use vulkano::pipeline::viewport::Viewport;
use vulkano::device::DeviceOwned;
use vulkano::pipeline::vertex::SingleBufferDefinition;
use vulkano::SafeDeref;
use vulkano::descriptor::PipelineLayoutAbstract;

pub mod culling;

use super::{Pipeline, PipelineError, pre_mul_alpha_blending};
use crate::renderer::{model, RenderPass};
use culling::{MMDCullMode, MMDCullModeEx};
use downcast_rs::__std::marker::PhantomData;

mod vert {
	#[allow(dead_code)]
	const SOURCE: &'static str = include_str!("./vert.glsl"); // https://github.com/vulkano-rs/vulkano/issues/1349
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/renderer/pipelines/mmd/vert.glsl"
	}
}

mod frag {
	#[allow(dead_code)]
	const SOURCE: &'static str = include_str!("./frag.glsl"); // https://github.com/vulkano-rs/vulkano/issues/1349
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/renderer/pipelines/mmd/frag.glsl"
	}
}

#[derive(Debug, Deref)]
pub struct MMDPipeline<Culling>(
	GraphicsPipeline<
		SingleBufferDefinition<model::mmd::Vertex>,
		Box<dyn PipelineLayoutAbstract + Send + Sync>,
		Arc<dyn RenderPassAbstract + Send + Sync>
	>,
	PhantomData<Culling>,
);

unsafe impl<Culling> SafeDeref for MMDPipeline<Culling> {} // MMDPipeline is immutable, this should be safe

impl<Culling> Pipeline for MMDPipeline<Culling>
	where Culling: MMDCullMode {
	fn new(render_pass: &Arc<RenderPass>, frame_buffer_size: (u32, u32)) -> Result<Arc<dyn Pipeline>, PipelineError> {
		let device = render_pass.device();
		let vs = vert::Shader::load(device.clone()).unwrap();
		let fs = frag::Shader::load(device.clone()).unwrap();
		
		Ok(Arc::new(MMDPipeline::<Culling>(
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
				.cull_mode::<Culling>()
				.blend_collective(pre_mul_alpha_blending())
				.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
				.build(device.clone())?,
			PhantomData
		)))
	}
}
