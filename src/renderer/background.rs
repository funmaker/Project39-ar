use std::sync::Arc;
use err_derive::Error;
use vulkano::image::AttachmentImage;
use vulkano::device::Queue;
use vulkano::buffer::{ImmutableBuffer, BufferUsage};
use vulkano::descriptor::{DescriptorSet, descriptor_set, PipelineLayoutAbstract};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::image::view::ImageView;
use vulkano::sampler::Sampler;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, DynamicState};
use vulkano::{memory, sync, command_buffer};

use super::pipelines::background::{BackgroundPipeline, Vertex};
use super::pipelines::{PipelineError, Pipelines};
use super::model::FenceCheck;

pub struct Background {
	pipeline: Arc<BackgroundPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	set: Arc<dyn DescriptorSet + Send + Sync>,
	fence: Arc<FenceCheck>,
}

impl Background {
	pub fn new(camera_image: Arc<AttachmentImage>, queue: &Arc<Queue>, pipelines: &mut Pipelines) -> Result<Background, BackgroundError> {
		let pipeline: Arc<BackgroundPipeline> = pipelines.get()?;
		
		let square = [
			Vertex::new([-1.0, -1.0]),
			Vertex::new([-1.0,  1.0]),
			Vertex::new([ 1.0, -1.0]),
			Vertex::new([ 1.0, -1.0]),
			Vertex::new([-1.0,  1.0]),
			Vertex::new([ 1.0,  1.0]),
		];
		
		let (vertices, vertices_promise) = ImmutableBuffer::from_iter(square.iter().cloned(),
		                                                              BufferUsage{ vertex_buffer: true, ..BufferUsage::none() },
		                                                              queue.clone())?;
		
		let view = ImageView::new(camera_image)?;
		let sampler = Sampler::simple_repeat_linear(queue.device().clone());
		
		let set = Arc::new(
			PersistentDescriptorSet::start(pipeline.descriptor_set_layout(0).ok_or(BackgroundError::NoLayout)?.clone())
				.add_sampled_image(view, sampler)?
				.build()?
		);
		
		let fence = Arc::new(FenceCheck::new(vertices_promise)?);
		
		Ok(Background {
			pipeline,
			vertices,
			set,
			fence
		})
	}
	
	pub fn render(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), BackgroundRenderError> {
		if !self.fence.check() { return Ok(()); }
		
		builder.draw(self.pipeline.clone(),
		             &DynamicState::none(),
		             self.vertices.clone(),
		             self.set.clone(),
		             (),
		             None)?;
		
		Ok(())
	}
}



#[derive(Debug, Error)]
pub enum BackgroundError {
	#[error(display = "Pipeline doesn't have specified layout")] NoLayout,
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] vulkano::image::ImageCreationError),
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] PersistentDescriptorSetError(#[error(source)] descriptor_set::PersistentDescriptorSetError),
	#[error(display = "{}", _0)] PersistentDescriptorSetBuildError(#[error(source)] descriptor_set::PersistentDescriptorSetBuildError),
}

#[derive(Debug, Error)]
pub enum BackgroundRenderError {
	#[error(display = "{}", _0)] DrawError(#[error(source)] command_buffer::DrawError),
}
