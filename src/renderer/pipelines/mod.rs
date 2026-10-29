use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use thiserror::Error;
use vulkano::device::{Device, DeviceOwned};
use vulkano::image::SampleCount;
use vulkano::pipeline::graphics::color_blend::{AttachmentBlend, BlendFactor, BlendOp, ColorBlendAttachmentState, ColorBlendState};
use vulkano::pipeline::graphics::depth_stencil::{DepthState, DepthStencilState};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexDefinition};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::graphics::GraphicsPipelineCreateInfo;
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;
use vulkano::pipeline::{ComputePipeline, DynamicState, GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo};
use vulkano::pipeline::compute::ComputePipelineCreateInfo;
use vulkano::render_pass::{RenderPass, Subpass};
use vulkano::shader::EntryPoint;

pub mod debug;
pub mod default;


pub trait PipelineConstructor: 'static {
	type PipeType: Any + Send + Sync;
	
	fn new(render_pass: &Arc<RenderPass>)
	      -> Result<Arc<Self::PipeType>>
	      where Self: Sized;
}

pub struct Pipelines {
	pipelines: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
	render_pass: Arc<RenderPass>,
}

impl Pipelines {
	pub fn new(render_pass: Arc<RenderPass>) -> Pipelines {
		Pipelines{
			pipelines: HashMap::new(),
			render_pass,
		}
	}
	
	pub fn get<P: PipelineConstructor>(&mut self) -> Result<Arc<P::PipeType>> {
		if let Some(pipeline) = self.pipelines.get(&TypeId::of::<P>()) {
			Ok(pipeline.clone().downcast().unwrap())
		} else {
			let pipeline = P::new(&self.render_pass)?;
			self.pipelines.insert(TypeId::of::<P>(), pipeline.clone());
			
			Ok(pipeline)
		}
	}
}


pub fn pre_mul_alpha_blending() -> AttachmentBlend {
	AttachmentBlend {
		color_blend_op: BlendOp::Add,
		src_color_blend_factor: BlendFactor::One,
		dst_color_blend_factor: BlendFactor::OneMinusSrcAlpha,
		alpha_blend_op: BlendOp::Add,
		src_alpha_blend_factor: BlendFactor::One,
		dst_alpha_blend_factor: BlendFactor::OneMinusSrcAlpha,
	}
}

pub fn simple_depth_test() -> DepthStencilState {
	DepthStencilState {
		depth: Some(DepthState::simple()),
		..DepthStencilState::default()
	}
}

pub fn build_graphics_pipeline<V>(render_pass: &Arc<RenderPass>,
                                  vs: EntryPoint,
                                  fs: EntryPoint,
                                  depth_stencil_state: DepthStencilState,
                                  rasterization_state: RasterizationState,
                                  blend: Option<AttachmentBlend>)
                                  -> Result<Arc<GraphicsPipeline>>
                                  where V: Vertex {
	let device = render_pass.device();
	let subpass = Subpass::from(render_pass.clone(), 0).ok_or_else(|| RenderPassNoSubpassError)?;
	let vertex_input_state = V::per_vertex().definition(&vs)?;
	
	let stages = smallvec::smallvec![
		PipelineShaderStageCreateInfo::new(vs),
		PipelineShaderStageCreateInfo::new(fs),
	];
	
	let layout = PipelineLayout::new(
		device.clone(),
		PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
			.into_pipeline_layout_create_info(device.clone())?,
	)?;
	
	Ok(GraphicsPipeline::new(device.clone(), None, GraphicsPipelineCreateInfo {
		stages,
		vertex_input_state: Some(vertex_input_state),
		input_assembly_state: Some(InputAssemblyState::default()),
		viewport_state: Some(ViewportState::default()),
		rasterization_state: Some(rasterization_state),
		multisample_state: Some(MultisampleState {
			rasterization_samples: subpass.num_samples().unwrap_or(SampleCount::Sample1),
			..MultisampleState::default()
		}),
		depth_stencil_state: Some(depth_stencil_state),
		color_blend_state: Some(ColorBlendState::with_attachment_states(
			subpass.num_color_attachments(),
			ColorBlendAttachmentState {
				blend,
				..ColorBlendAttachmentState::default()
			}),
		),
		dynamic_state: [DynamicState::Viewport].into_iter().collect(),
		subpass: Some(subpass.into()),
		..GraphicsPipelineCreateInfo::layout(layout)
	})?)
}

pub fn build_compute_pipeline(device: &Arc<Device>, cs: EntryPoint) -> Result<Arc<ComputePipeline>> {
	let stage = PipelineShaderStageCreateInfo::new(cs);
	
	let layout = PipelineLayout::new(
		device.clone(),
		PipelineDescriptorSetLayoutCreateInfo::from_stages(Some(&stage))
			.into_pipeline_layout_create_info(device.clone())?,
	)?;
	
	Ok(ComputePipeline::new(
		device.clone(),
		None,
		ComputePipelineCreateInfo::stage_layout(stage, layout))?,
	)
}

#[derive(Debug, Error)]
#[error("Pipeline doesn't have specified layout")]
pub struct PipelineNoLayoutError;

#[derive(Debug, Error)]
#[error("RenderPass doesn't have specified Subpass")]
pub struct RenderPassNoSubpassError;
