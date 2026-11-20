use std::convert::TryInto;
use std::sync::Arc;
use anyhow::Result;
pub use frag::Params;
use vulkano::device::DeviceOwned;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState};
use vulkano::pipeline::graphics::vertex_input::{Vertex as VertexTy, VertexDefinition};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::{RenderPass, Subpass};

mod vertex;

use crate::renderer::pipelines::{build_graphics_pipeline, PipelineConstructor};
pub use vertex::{Vertex, MeshVertex};


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

mod mesh_vert {
	vulkano_shaders::shader! {
		ty: "vertex",
		path: "src/application/eyes/pipeline/mesh_vert.glsl",
		spirv_version: "1.3"
	}
}

mod mesh_frag {
	vulkano_shaders::shader! {
		ty: "fragment",
		path: "src/application/eyes/pipeline/mesh_frag.glsl",
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

/// 3D passthrough (depth mesh), drawn over `BackgroundPipeline`.
pub struct BackgroundMeshPipeline;

impl PipelineConstructor for BackgroundMeshPipeline {
	type PipeType = GraphicsPipeline;

	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();

		build_graphics_pipeline::<MeshVertex>(
			render_pass,
			mesh_vert::load(device.clone())?.entry_point("main").unwrap(),
			mesh_frag::load(device.clone())?.entry_point("main").unwrap(),
			DepthStencilState::default(),
			RasterizationState { cull_mode: CullMode::None, ..RasterizationState::default() },
			None,
		)
	}
}
