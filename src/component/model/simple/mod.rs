use std::sync::Arc;
use image::{DynamicImage, GenericImageView};
use num_traits::FromPrimitive;
use simba::scalar::SubsetOf;
use openvr::render_models;
use vulkano::buffer::{ImmutableBuffer, BufferUsage, TypedBufferAccess};
use vulkano::image::{ImmutableImage, MipmapsCount, ImageDimensions, view::ImageView};
use vulkano::sync::GpuFuture;
use vulkano::descriptor_set::{DescriptorSet, PersistentDescriptorSet};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::format::Format;
use vulkano::sampler::Sampler;
use vulkano::pipeline::{GraphicsPipeline, PipelineBindPoint};

mod import;

pub use crate::renderer::pipelines::default::Vertex;
use crate::renderer::pipelines::default::DefaultPipeline;
use crate::renderer::Renderer;
use crate::utils::{ImageEx, FenceCheck};
use crate::math::AMat4;
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::application::Entity;
use super::{ModelError, VertexIndex};
pub use import::SimpleModelLoadError;

#[derive(ComponentBase, Clone)]
pub struct SimpleModel<VI: VertexIndex> {
	#[inner] inner: ComponentInner,
	pipeline: Arc<GraphicsPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	indices: Arc<ImmutableBuffer<[VI]>>,
	set: Arc<dyn DescriptorSet + Send + Sync>,
	fence: FenceCheck,
}

#[allow(dead_code)]
impl<VI: VertexIndex + FromPrimitive> SimpleModel<VI> {
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
		                                                       ImageDimensions::Dim2d{ width, height, array_layers: 1 },
		                                                       MipmapsCount::Log2,
		                                                       Format::R8G8B8A8_UNORM,
		                                                       queue.clone())?;
		
		let view = ImageView::new(image)?;
		let sampler = Sampler::simple_repeat_linear(queue.device().clone());
		
		let set = {
			let mut set_builder = PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layouts().get(0).ok_or(ModelError::NoLayout)?.clone());
			set_builder.add_buffer(renderer.commons.clone())?
			           .add_sampled_image(view, sampler)?;
			Arc::new(set_builder.build()?)
		};
		
		let fence = FenceCheck::new(vertices_promise.join(indices_promise).join(image_promise))?;
		
		Ok(SimpleModel {
			inner: ComponentInner::new(),
			pipeline,
			vertices,
			indices,
			set,
			fence,
		})
	}
	
	pub fn from_obj(path: &str, renderer: &mut Renderer) -> Result<SimpleModel<VI>, SimpleModelLoadError> {
		import::from_obj(path, renderer)
	}
	
	pub fn from_openvr(model: render_models::Model, texture: render_models::Texture, renderer: &mut Renderer) -> Result<SimpleModel<u16>, SimpleModelLoadError> {
		import::from_openvr(model, texture, renderer)
	}
	
	pub fn loaded(&self) -> bool {
		self.fence.check()
	}
	
	// pub fn try_clone(&self, _renderer: &mut Renderer) -> Result<Box<dyn Model>, ModelError> {
	// 	Ok(Box::new(self.clone()))
	// }
}

impl<VI: VertexIndex + FromPrimitive> Component for SimpleModel<VI> {
	fn render(&self, entity: &Entity, _renderer: &Renderer, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		if !self.loaded() { return Ok(()) }
		
		let model_matrix: AMat4 = entity.state().position.to_superset();
		
		builder.bind_pipeline_graphics(self.pipeline.clone())
		       .bind_vertex_buffers(0, self.vertices.clone())
		       .bind_index_buffer(self.indices.clone())
		       .bind_descriptor_sets(PipelineBindPoint::Graphics,
		                             self.pipeline.layout().clone(),
		                             0,
		                             self.set.clone())
		       .push_constants(self.pipeline.layout().clone(),
		                       0,
		                       model_matrix.to_homogeneous())
		       .draw_indexed(self.indices.len() as u32,
		                     1,
		                     0,
		                     0,
		                     0)?;
		
		Ok(())
	}
}


