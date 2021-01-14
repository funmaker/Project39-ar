use std::sync::Arc;
use cgmath::Matrix4;
use image::{DynamicImage, GenericImageView};
use vulkano::buffer::{ImmutableBuffer, BufferUsage};
use vulkano::image::{ImmutableImage, Dimensions, MipmapsCount};
use vulkano::sync::GpuFuture;
use vulkano::descriptor::{DescriptorSet, PipelineLayoutAbstract};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState};
use vulkano::format::Format;
use vulkano::sampler::Sampler;

use crate::renderer::{Renderer, RendererRenderError};
use crate::renderer::pipelines::default::DefaultPipeline;
use crate::utils::ImageEx;
use super::{Model, ModelError, VertexIndex, FenceCheck};
pub use crate::renderer::pipelines::default::Vertex;

pub struct SimpleModel<VI: VertexIndex> {
	pipeline: Arc<DefaultPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	indices: Arc<ImmutableBuffer<[VI]>>,
	set: Arc<dyn DescriptorSet + Send + Sync>,
	fence: FenceCheck,
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
		
		let (image, image_promise) = ImmutableImage::from_iter(source_image.into_pre_mul_iter(),
		                                                       Dimensions::Dim2d{ width, height },
		                                                       MipmapsCount::Log2,
		                                                       Format::R8G8B8A8Unorm,
		                                                       queue.clone())?;
		
		let sampler = Sampler::simple_repeat_linear(queue.device().clone());
		
		let set = Arc::new(
			PersistentDescriptorSet::start(pipeline.descriptor_set_layout(0).ok_or(ModelError::NoLayout)?.clone())
			                             .add_buffer(renderer.commons.clone())?
			                             .add_sampled_image(image.clone(), sampler.clone())?
			                             .build()?
		);
		
		let fence = FenceCheck::new(vertices_promise.join(indices_promise).join(image_promise))?;
		
		Ok(SimpleModel {
			pipeline,
			vertices,
			indices,
			set,
			fence,
		})
	}
	
	pub fn loaded(&self) -> bool {
		self.fence.check()
	}
}

impl<VI: VertexIndex> Model for SimpleModel<VI> {
	fn render(&self, builder: &mut AutoCommandBufferBuilder, model_matrix: Matrix4<f32>, eye: u32) -> Result<(), RendererRenderError> {
		if !self.loaded() { return Ok(()) }
		
		builder.draw_indexed(self.pipeline.clone(),
		                     &DynamicState::none(),
		                     self.vertices.clone(),
		                     self.indices.clone(),
		                     self.set.clone(),
		                     (model_matrix, eye))?;
		
		Ok(())
	}
}


