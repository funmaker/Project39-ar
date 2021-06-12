use std::sync::Arc;
use err_derive::Error;
use vulkano::image::AttachmentImage;
use vulkano::device::Queue;
use vulkano::buffer::{ImmutableBuffer, BufferUsage};
use vulkano::descriptor::{DescriptorSet, descriptor_set};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::image::view::ImageView;
use vulkano::sampler::{Sampler, Filter, MipmapMode, SamplerAddressMode};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, DynamicState};
use vulkano::sync::GpuFuture;
use vulkano::{memory, sync, command_buffer, sampler};

use super::pipelines::background::{BackgroundPipeline, Vertex};
use super::pipelines::{PipelineError, Pipelines};
use super::model::FenceCheck;
use crate::math::{Vec4, Vec2};
use crate::config;
use vulkano::pipeline::GraphicsPipelineAbstract;

#[allow(dead_code)]
#[derive(Copy, Clone)]
struct Intrinsics {
	hfov: [Vec2; 2],
	dfov: [Vec2; 2],
	coeffs: [Vec4; 2],
	scale: [Vec2; 2],
	center: [Vec2; 2],
}

pub struct Background {
	pipeline: Arc<BackgroundPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	set: Arc<dyn DescriptorSet + Send + Sync>,
	fence: Arc<FenceCheck>,
}

impl Background {
	pub fn new(camera_image: Arc<AttachmentImage>, queue: &Arc<Queue>, pipelines: &mut Pipelines) -> Result<Background, BackgroundError> {
		let config = config::get();
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
		
		let hfov = [
			config.camera.left.size.cast::<f32>().component_div(&config.camera.left.focal_length) / 2.0,
			config.camera.right.size.cast::<f32>().component_div(&config.camera.right.focal_length) / 2.0,
		];
		
		let mut dfov = hfov;
		
		
		
		let intrinsics = Intrinsics {
			hfov,
			dfov,
			coeffs: [
				config.camera.left.coeffs.clone(),
				config.camera.right.coeffs.clone(),
			],
			scale: [
				config.camera.left.size.cast::<f32>().component_div(&config.camera.frame_buffer_size.cast()),
				config.camera.right.size.cast::<f32>().component_div(&config.camera.frame_buffer_size.cast()),
			],
			center: [
				config.camera.left.center.component_div(&config.camera.frame_buffer_size.cast()),
				config.camera.right.center.component_div(&config.camera.frame_buffer_size.cast()),
			],
		};
		
		let (intrinsics, intrinsics_promise) = ImmutableBuffer::from_data(intrinsics,
		                                                                  BufferUsage{ uniform_buffer: true, ..BufferUsage::none() },
		                                                                  queue.clone())?;
		
		let view = ImageView::new(camera_image)?;
		let sampler = Sampler::new(
			queue.device().clone(),
			Filter::Linear,
			Filter::Linear,
			MipmapMode::Nearest,
			SamplerAddressMode::ClampToEdge,
			SamplerAddressMode::ClampToEdge,
			SamplerAddressMode::ClampToEdge,
			0.0,
			1.0,
			0.0,
			1.0,
		)?;
		
		let set = Arc::new(
			PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layout(0).ok_or(BackgroundError::NoLayout)?.clone())
				.add_buffer(intrinsics.clone())?
				.add_sampled_image(view, sampler)?
				.build()?
		);
		
		let fence = Arc::new(FenceCheck::new(vertices_promise.join(intrinsics_promise))?);
		
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
	#[error(display = "{}", _0)] SamplerCreationError(#[error(source)] sampler::SamplerCreationError),
}

#[derive(Debug, Error)]
pub enum BackgroundRenderError {
	#[error(display = "{}", _0)] DrawError(#[error(source)] command_buffer::DrawError),
}
