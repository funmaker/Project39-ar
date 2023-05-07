use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use err_derive::Error;
use vulkano::pipeline::graphics::color_blend::{AttachmentBlend, BlendFactor, BlendOp};
use vulkano::render_pass::RenderPass;

pub mod debug;
pub mod default;


pub trait PipelineConstructor: 'static {
	type PipeType: Any + Send + Sync;
	
	fn new(render_pass: &Arc<RenderPass>)
	      -> Result<Arc<Self::PipeType>, PipelineError>
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
	
	pub fn get<P: PipelineConstructor>(&mut self) -> Result<Arc<P::PipeType>, PipelineError> {
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
		color_op: BlendOp::Add,
		color_source: BlendFactor::One,
		color_destination: BlendFactor::OneMinusSrcAlpha,
		alpha_op: BlendOp::Add,
		alpha_source: BlendFactor::One,
		alpha_destination: BlendFactor::OneMinusSrcAlpha,
	}
}


#[derive(Debug, Error)]
pub enum PipelineError {
	#[error(display = "{}", _0)] GraphicsPipelineCreationError(#[error(source)] vulkano::pipeline::graphics::GraphicsPipelineCreationError),
	#[error(display = "{}", _0)] ComputePipelineCreationError(#[error(source)] vulkano::pipeline::compute::ComputePipelineCreationError),
}
