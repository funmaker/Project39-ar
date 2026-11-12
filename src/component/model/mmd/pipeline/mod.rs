pub use outline_vert::Pc;
use std::sync::Arc;
use anyhow::Result;
use vulkano::device::{Device, DeviceOwned};
use vulkano::pipeline::graphics::rasterization::{CullMode, FrontFace, RasterizationState};
use vulkano::pipeline::{ComputePipeline, GraphicsPipeline};
use vulkano::render_pass::RenderPass;
use vulkano::shader::{EntryPoint, SpecializationConstant, ShaderModule};

#[macro_use] mod macros;
mod vertex;

use crate::renderer::pipelines::{build_compute_pipeline, build_graphics_pipeline, pre_mul_alpha_blending, simple_depth_test, PipelineConstructor};
pub use vertex::Vertex;


mmd_shaders!(
	"vertex" base_vert = "src/component/model/mmd/pipeline/base_vert.glsl";
	"fragment" base_frag = "src/component/model/mmd/pipeline/base_frag.glsl";
	"vertex" outline_vert = "src/component/model/mmd/pipeline/outline_vert.glsl";
	"fragment" outline_frag = "src/component/model/mmd/pipeline/outline_frag.glsl";
	"compute" morph_comp = "src/component/model/mmd/pipeline/morph_comp.glsl";
);

/// Loads `base_frag` with the `transparent_pass` specialization constant (`constant_id = 0`) set.
fn base_fragment(device: &Arc<Device>, transparent_pass: bool) -> EntryPoint {
	use foldhash::HashMapExt;
	
	let module: Arc<ShaderModule> = base_frag::load(device.clone()).unwrap();
	
	let mut specializations: foldhash::HashMap<u32, SpecializationConstant> = foldhash::HashMap::new();
	specializations.insert(0u32, SpecializationConstant::Bool(transparent_pass));
	
	let module = module.specialize(specializations).unwrap();
	module.entry_point("main").unwrap()
}

/// MMD material pipeline for the opaque pass, with back face culling.
pub struct MMDPipelineOpaque;

impl PipelineConstructor for MMDPipelineOpaque {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();
		
		build_graphics_pipeline::<Vertex>(
			render_pass,
			base_vert::load(device.clone())?.entry_point("main").unwrap(),
			base_fragment(device, false),
			simple_depth_test(),
			RasterizationState {
				cull_mode: CullMode::Back,
				front_face: FrontFace::Clockwise,
				..RasterizationState::default()
			},
			if false { Some(pre_mul_alpha_blending()) } else { None },
		)
	}
}

/// MMD material pipeline for the opaque pass, for double sided materials.
pub struct MMDPipelineOpaqueNoCull;

impl PipelineConstructor for MMDPipelineOpaqueNoCull {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();
		
		build_graphics_pipeline::<Vertex>(
			render_pass,
			base_vert::load(device.clone())?.entry_point("main").unwrap(),
			base_fragment(device, false),
			simple_depth_test(),
			RasterizationState {
				cull_mode: CullMode::None,
				front_face: FrontFace::Clockwise,
				..RasterizationState::default()
			},
			if false { Some(pre_mul_alpha_blending()) } else { None },
		)
	}
}

/// MMD material pipeline for the transparent pass, with back face culling.
pub struct MMDPipelineTrans;

impl PipelineConstructor for MMDPipelineTrans {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();
		
		build_graphics_pipeline::<Vertex>(
			render_pass,
			base_vert::load(device.clone())?.entry_point("main").unwrap(),
			base_fragment(device, true),
			simple_depth_test(),
			RasterizationState {
				cull_mode: CullMode::Back,
				front_face: FrontFace::Clockwise,
				..RasterizationState::default()
			},
			if true { Some(pre_mul_alpha_blending()) } else { None },
		)
	}
}

/// MMD material pipeline for the transparent pass, for double sided materials.
pub struct MMDPipelineTransNoCull;

impl PipelineConstructor for MMDPipelineTransNoCull {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();
		
		build_graphics_pipeline::<Vertex>(
			render_pass,
			base_vert::load(device.clone())?.entry_point("main").unwrap(),
			base_fragment(device, true),
			simple_depth_test(),
			RasterizationState {
				cull_mode: CullMode::None,
				front_face: FrontFace::Clockwise,
				..RasterizationState::default()
			},
			if true { Some(pre_mul_alpha_blending()) } else { None },
		)
	}
}

/// Pipeline drawing MMD edges (toon outlines) as inflated back faces.
pub struct MMDPipelineOutline;

impl PipelineConstructor for MMDPipelineOutline {
	type PipeType = GraphicsPipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device();
		
		build_graphics_pipeline::<Vertex>(
			render_pass,
			outline_vert::load(device.clone())?.entry_point("main").unwrap(),
			outline_frag::load(device.clone())?.entry_point("main").unwrap(),
			simple_depth_test(),
			RasterizationState {
				cull_mode: CullMode::Back,
				front_face: FrontFace::CounterClockwise,
				..RasterizationState::default()
			},
			if true { Some(pre_mul_alpha_blending()) } else { None },
		)
	}
}

/// Workgroup size of the morph compute shader.
pub const MORPH_GROUP_SIZE: usize = 32;

/// Compute pipeline applying vertex morphs into the morphed vertex offsets buffer.
pub struct MMDPipelineMorphs;

impl PipelineConstructor for MMDPipelineMorphs {
	type PipeType = ComputePipeline;
	
	fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>> {
		let device = render_pass.device().clone();
		let cs = morph_comp::load(device.clone())?.entry_point("main").unwrap();
		
		build_compute_pipeline(&device, cs)
	}
}
