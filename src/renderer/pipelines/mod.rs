use std::collections::HashMap;
use std::sync::Arc;
use std::any::TypeId;
use std::fmt::Debug;
use err_derive::Error;
use downcast_rs::{DowncastSync, impl_downcast};
use vulkano::framebuffer::RenderPassCreationError;
use vulkano::pipeline::GraphicsPipelineCreationError;

mod default;
mod mmd;

pub use default::DefaultPipeline;
pub use self::mmd::MMDPipeline;
use super::RenderPass;

pub trait Pipeline: DowncastSync + Debug {
	fn new(render_pass: &Arc<RenderPass>, frame_buffer_size: (u32, u32))
	      -> Result<Arc<dyn Pipeline>, PipelineError>
	      where Self: Sized;
}
impl_downcast!(sync Pipeline);

pub struct Pipelines {
	pipelines: HashMap<TypeId, Arc<dyn Pipeline>>,
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
	
	pub fn get<P: Pipeline>(&mut self) -> Result<Arc<P>, PipelineError> {
		if let Some(pipeline) = self.pipelines.get(&TypeId::of::<P>()) {
			Ok(pipeline.clone().downcast_arc().unwrap())
		} else {
			let pipeline = P::new(&self.render_pass, self.frame_buffer_size)?;
			self.pipelines.insert(TypeId::of::<P>(), pipeline.clone());
			
			Ok(pipeline.downcast_arc().unwrap())
		}
	}
}


#[derive(Debug, Error)]
pub enum PipelineError {
	#[error(display = "{}", _0)] RenderPassCreationError(#[error(source)] RenderPassCreationError),
	#[error(display = "{}", _0)] GraphicsPipelineCreationError(#[error(source)] GraphicsPipelineCreationError),
}
