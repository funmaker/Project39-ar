use std::sync::Arc;
use std::time::Duration;
use arc_swap::ArcSwap;
use cgmath::Matrix4;
use image::{DynamicImage, GenericImageView};
use vulkano::buffer::{ImmutableBuffer, BufferUsage};
use vulkano::image::{ImmutableImage, Dimensions};
use vulkano::sync::{GpuFuture, FlushError, FenceSignalFuture};
use vulkano::descriptor::{DescriptorSet, PipelineLayoutAbstract};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState};
use vulkano::format::Format;
use vulkano::sampler::Sampler;

mod vertex;

pub use vertex::Vertex;
use crate::renderer::{Renderer, RenderError};
use crate::renderer::pipelines::DefaultPipeline;
use super::{Model, ModelError, VertexIndex};

pub struct SimpleModel<VI: VertexIndex> {
	pipeline: Arc<DefaultPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	indices: Arc<ImmutableBuffer<[VI]>>,
	set: Arc<dyn DescriptorSet + Send + Sync>,
	fence: ArcSwap<FenceCheck>,
}

impl<VI: VertexIndex> SimpleModel<VI> {
	pub fn new(vertices: &[Vertex], indices: &[VI], source_image: DynamicImage, renderer: &mut Renderer) -> Result<SimpleModel<VI>, ModelError> {
		let width = source_image.width();
		let height = source_image.height();
		let queue = &renderer.load_queue;
		
		let pipeline = renderer.pipelines.get::<DefaultPipeline>()?;
		
		let (vertices, vertices_promise) = ImmutableBuffer::from_iter(vertices.iter().cloned(),
		                                                              BufferUsage{ vertex_buffer: true, ..BufferUsage::none() },
		                                                              queue.clone())?;
		
		let (indices, indices_promise) = ImmutableBuffer::from_iter(indices.iter().copied(),
		                                                            BufferUsage{ index_buffer: true, ..BufferUsage::none() },
		                                                            queue.clone())?;
		
		let (image, image_promise) = ImmutableImage::from_iter(source_image.to_rgba8().into_vec().into_iter(),
		                                                       Dimensions::Dim2d{ width, height },
		                                                       Format::R8G8B8A8Unorm,
		                                                       queue.clone())?;
		
		let sampler = Sampler::simple_repeat_linear_no_mipmap(queue.device().clone());
		
		let set = Arc::new(
			PersistentDescriptorSet::start(pipeline.descriptor_set_layout(0).ok_or(ModelError::NoLayout)?.clone())
				.add_sampled_image(image.clone(), sampler.clone())?
				.build()?
		);
		
		let fence = ArcSwap::new(Arc::new(FenceCheck::new(vertices_promise.join(indices_promise).join(image_promise))?));
		
		Ok(SimpleModel {
			pipeline,
			vertices,
			indices,
			set,
			fence,
		})
	}
	
	pub fn loaded(&self) -> bool {
		match &**self.fence.load() {
			FenceCheck::Done(result) => *result,
			FenceCheck::Pending(fence) => {
				match fence.wait(Some(Duration::new(0, 0))) {
					Err(FlushError::Timeout) => false,
					Ok(()) => {
						self.fence.swap(Arc::new(FenceCheck::Done(true)));
						true
					}
					Err(err) => {
						eprintln!("Error while loading renderer.model: {:?}", err);
						self.fence.swap(Arc::new(FenceCheck::Done(false)));
						false
					}
				}
			}
		}
	}
}

impl<VI: VertexIndex> Model for SimpleModel<VI> {
	fn render(&self, builder: &mut AutoCommandBufferBuilder, pvm_matrix: Matrix4<f32>) -> Result<(), RenderError> {
		if !self.loaded() { return Ok(()) }
		
		builder.draw_indexed(self.pipeline.clone(),
		                     &DynamicState::none(),
		                     self.vertices.clone(),
		                     self.indices.clone(),
		                     self.set.clone(),
		                     pvm_matrix)?;
		
		Ok(())
	}
}

enum FenceCheck {
	Done(bool),
	Pending(FenceSignalFuture<Box<dyn GpuFuture>>)
}

impl FenceCheck {
	fn new<GF>(future: GF)
	           -> Result<FenceCheck, FlushError>
		where GF: GpuFuture + 'static {
		Ok(FenceCheck::Pending((Box::new(future) as Box<dyn GpuFuture>).then_signal_fence_and_flush()?))
	}
}
