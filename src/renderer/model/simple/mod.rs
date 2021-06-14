use std::sync::Arc;
use image::{DynamicImage, GenericImageView};
use num_traits::FromPrimitive;
use vulkano::buffer::{ImmutableBuffer, BufferUsage};
use vulkano::image::{ImmutableImage, MipmapsCount, ImageDimensions, view::ImageView};
use vulkano::sync::GpuFuture;
use vulkano::descriptor::DescriptorSet;
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState, PrimaryAutoCommandBuffer};
use vulkano::format::Format;
use vulkano::sampler::Sampler;
use openvr::render_models;

mod import;

pub use crate::renderer::pipelines::default::Vertex;
use crate::renderer::pipelines::default::DefaultPipeline;
use crate::renderer::Renderer;
use crate::utils::ImageEx;
use crate::math::AMat4;
use super::{Model, ModelError, ModelRenderError, VertexIndex, FenceCheck};
pub use import::SimpleModelLoadError;
use vulkano::pipeline::GraphicsPipelineAbstract;

#[derive(Clone)]
pub struct SimpleModel<VI: VertexIndex> {
	pipeline: Arc<DefaultPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	indices: Arc<ImmutableBuffer<[VI]>>,
	set: Arc<dyn DescriptorSet + Send + Sync>,
	fence: Arc<FenceCheck>,
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
		                                                       Format::R8G8B8A8Unorm,
		                                                       queue.clone())?;
		
		let view = ImageView::new(image)?;
		let sampler = Sampler::simple_repeat_linear(queue.device().clone());
		
		let set = Arc::new(
			PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layout(0).ok_or(ModelError::NoLayout)?.clone())
			                        .add_buffer(renderer.commons.clone())?
			                        .add_sampled_image(view, sampler)?
			                        .build()?
		);
		
		let fence = Arc::new(FenceCheck::new(vertices_promise.join(indices_promise).join(image_promise))?);
		
		Ok(SimpleModel {
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
}

impl<VI: VertexIndex + FromPrimitive> Model for SimpleModel<VI> {
	fn render(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, model_matrix: &AMat4) -> Result<(), ModelRenderError> {
		if !self.loaded() { return Ok(()) }
		
		builder.draw_indexed(self.pipeline.clone(),
		                     &DynamicState::none(),
		                     self.vertices.clone(),
		                     self.indices.clone(),
		                     self.set.clone(),
		                     model_matrix.to_homogeneous(),
		                     None)?;
		
		Ok(())
	}
	
	fn try_clone(&self, _renderer: &mut Renderer) -> Result<Box<dyn Model>, ModelError> {
		Ok(Box::new(self.clone()))
	}
}


