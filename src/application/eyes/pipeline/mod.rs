use std::convert::TryInto;
use std::sync::Arc;
use anyhow::Result;
pub use frag::Pc;
use vulkano::device::DeviceOwned;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState};
use vulkano::pipeline::graphics::vertex_input::{Vertex as VertexTy, VertexDefinition};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{RenderPass, Subpass};

mod vertex;

use crate::renderer::pipelines::{build_graphics_pipeline, PipelineConstructor};
pub use vertex::Vertex;


mod vert {
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/application/eyes/pipeline/vert.glsl",
		spirv_version: "1.3"
	}
}

mod frag {
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/application/eyes/pipeline/frag.glsl",
		spirv_version: "1.3"
	}
}

pub struct BackgroundPipeline;

impl PipelineConstructor for BackgroundPipeline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();
		
		build_graphics_pipeline::<Vertex>(
			render_pass,
			vert::load(device.clone())?.entry_point("main").unwrap(),
			frag::load(device.clone())?.entry_point("main").unwrap(),
			DepthStencilState::default(),
			RasterizationState { cull_mode: CullMode::Back, ..RasterizationState::default() },
			None,
		)
	}
}
