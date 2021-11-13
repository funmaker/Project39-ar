use std::collections::HashMap;
use std::sync::Arc;
use std::any::{TypeId, Any};
use std::fmt::Debug;
use err_derive::Error;
use vulkano::pipeline::blend::{AttachmentBlend, BlendOp, BlendFactor};
use vulkano::render_pass::RenderPass;

pub mod default;
pub mod mmd;
pub mod debug;
pub mod toolgun_text;

pub trait PipelineConstructor: 'static {
	type PipeType: Any + Send + Sync;
	
	fn new(render_pass: &Arc<RenderPass>, frame_buffer_size: (u32, u32))
	      -> Result<Arc<Self::PipeType>, PipelineError>
	      where Self: Sized;
}

pub struct Pipelines {
	pipelines: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
	render_pass: Arc<RenderPass>,
	frame_buffer_size: (u32, u32),
}

impl Pipelines {
	pub fn new(render_pass: Arc<RenderPass>, frame_buffer_size: (u32, u32)) -> Pipelines {
		Pipelines{
			pipelines: HashMap::new(),
			render_pass,
			frame_buffer_size,
		}
	}
	
	pub fn get<P: PipelineConstructor>(&mut self) -> Result<Arc<P::PipeType>, PipelineError> {
		if let Some(pipeline) = self.pipelines.get(&TypeId::of::<P>()) {
			Ok(pipeline.clone().downcast().unwrap())
		} else {
			let pipeline = P::new(&self.render_pass, self.frame_buffer_size)?;
			self.pipelines.insert(TypeId::of::<P>(), pipeline.clone());
			
			Ok(pipeline)
		}
	}
}


pub fn pre_mul_alpha_blending() -> AttachmentBlend {
	AttachmentBlend {
		enabled: true,
		color_op: BlendOp::Add,
		color_source: BlendFactor::One,
		color_destination: BlendFactor::OneMinusSrcAlpha,
		alpha_op: BlendOp::Add,
		alpha_source: BlendFactor::One,
		alpha_destination: BlendFactor::OneMinusSrcAlpha,
		mask_red: true,
		mask_green: true,
		mask_blue: true,
		mask_alpha: true,
	}
}


#[derive(Debug, Error)]
pub enum PipelineError {
	#[error(display = "{}", _0)] RenderPassCreationError(#[error(source)] vulkano::render_pass::RenderPassCreationError),
	#[error(display = "{}", _0)] GraphicsPipelineCreationError(#[error(source)] vulkano::pipeline::GraphicsPipelineCreationError),
	#[error(display = "{}", _0)] ComputePipelineCreationError(#[error(source)] vulkano::pipeline::ComputePipelineCreationError),
}
