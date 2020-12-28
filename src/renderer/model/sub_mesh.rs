use std::sync::Arc;
use cgmath::Matrix4;
use image::{DynamicImage, GenericImageView};
use vulkano::buffer::{ImmutableBuffer, BufferUsage};
use vulkano::image::{ImmutableImage, Dimensions};
use vulkano::command_buffer::{DynamicState, AutoCommandBufferBuilder};
use vulkano::descriptor::{DescriptorSet, PipelineLayoutAbstract};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::sampler::Sampler;
use vulkano::sync::GpuFuture;
use vulkano::format::Format;

use super::{Model, VertexIndex, ModelError, Renderer};
use crate::renderer::{PipelineType, RenderError};

pub struct SubMesh {
	indices: Arc<ImmutableBuffer<[VertexIndex]>>,
	_image: Arc<ImmutableImage<Format>>,
	set: Arc<dyn DescriptorSet + Send + Sync>,
}

impl SubMesh {
	pub fn new(indices: &[VertexIndex], source_image: DynamicImage, renderer: &Renderer) -> Result<(SubMesh, impl GpuFuture), ModelError> {
		let queue = &renderer.load_queue;
		let width = source_image.width();
		let height = source_image.height();
		
		let (indices, indices_promise) = ImmutableBuffer::from_iter(indices.iter().cloned(),
		                                                            BufferUsage{ index_buffer: true, ..BufferUsage::none() },
		                                                            queue.clone())?;
		
		let (image, image_promise) = ImmutableImage::from_iter(source_image.to_rgba8().into_vec().into_iter(),
		                                                       Dimensions::Dim2d{ width, height },
		                                                       Format::R8G8B8A8Unorm,
		                                                       queue.clone())?;
		
		let sampler = Sampler::simple_repeat_linear_no_mipmap(queue.device().clone());
		
		let set = Arc::new(PersistentDescriptorSet::start(renderer.pipeline.descriptor_set_layout(0).ok_or(ModelError::NoLayout)?.clone())
			                                       .add_sampled_image(image.clone(), sampler.clone())?
			                                       .build()?);
		
		let sub_mesh = SubMesh { indices, _image: image, set };
		let future = indices_promise.join(image_promise);
		
		Ok((sub_mesh, future))
	}
	
	pub fn render(&self, model: &Model, builder: &mut AutoCommandBufferBuilder, pipeline: &Arc<PipelineType>, pvm_matrix: Matrix4<f32>) -> Result<(), RenderError> {
		builder.draw_indexed(pipeline.clone(),
		                     &DynamicState::none(),
		                     model.vertices.clone(),
		                     self.indices.clone(),
		                     self.set.clone(),
		                     pvm_matrix)?;
		
		Ok(())
	}
}
