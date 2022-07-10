use std::sync::Arc;
use vulkano::buffer::{ImmutableBuffer, BufferUsage};
use vulkano::sync::GpuFuture;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::pipeline::{Pipeline, GraphicsPipeline, PipelineBindPoint};

pub use crate::renderer::pipelines::default::Vertex;
use crate::renderer::pipelines::default::DefaultPipeline;
use crate::renderer::Renderer;
use crate::utils::{FenceCheck, ImmutableIndexBuffer, AutoCommandBufferBuilderEx};
use crate::math::{Similarity3, Color, Point3, AABB, aabb_from_points};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError, RenderType};
use crate::application::Entity;
use crate::renderer::assets_manager::texture::TextureBundle;
use super::{ModelError, VertexIndex};

#[derive(ComponentBase, Clone)]
pub struct SimpleModel {
	#[inner] inner: ComponentInner,
	aabb: AABB,
	pipeline: Arc<GraphicsPipeline>,
	pub vertices: Arc<ImmutableBuffer<[Vertex]>>,
	pub indices: ImmutableIndexBuffer,
	pub set: Arc<PersistentDescriptorSet>,
	pub fence: FenceCheck,
}

#[allow(dead_code)]
impl SimpleModel {
	pub fn new<VI>(vertices: &[Vertex],
	               indices: &[VI],
	               texture: TextureBundle,
	               renderer: &mut Renderer)
	               -> Result<SimpleModel, ModelError>
	               where VI: VertexIndex {
		let aabb = aabb_from_points(vertices.iter().map(|v| Point3::from(v.pos)));
		let pipeline = renderer.pipelines.get::<DefaultPipeline>()?;
		
		let (vertices, vertices_promise) = ImmutableBuffer::from_iter(vertices.iter().cloned(),
		                                                              BufferUsage{ vertex_buffer: true, ..BufferUsage::none() },
		                                                              renderer.load_queue.clone())?;
		
		let (indices, indices_promise) = ImmutableBuffer::from_iter(indices.iter().copied(),
		                                                            BufferUsage{ index_buffer: true, ..BufferUsage::none() },
		                                                            renderer.load_queue.clone())?;
		
		let set = PersistentDescriptorSet::new(pipeline.layout().set_layouts().get(0).ok_or(ModelError::NoLayout)?.clone(), [
			WriteDescriptorSet::buffer(0, renderer.commons.clone()),
			WriteDescriptorSet::image_view_sampler(1, texture.view.clone(), texture.sampler.clone()),
		])?;
		
		let fence = FenceCheck::new(vertices_promise.join(indices_promise).join(texture.fence.future()))?;
		
		Ok(SimpleModel {
			inner: ComponentInner::from_render_type(RenderType::Opaque),
			aabb,
			pipeline,
			vertices,
			indices: indices.into(),
			set,
			fence,
		})
	}
	
	pub fn loaded(&self) -> bool {
		self.fence.check()
	}
	
	// pub fn try_clone(&self, _renderer: &mut Renderer) -> Result<Box<dyn Model>, ModelError> {
	// 	Ok(Box::new(self.clone()))
	// }
	
	pub fn aabb(&self) -> AABB {
		self.aabb
	}
	
	pub fn render_impl(&self, transform: Similarity3, color: Color, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		if !self.loaded() { return Ok(()) }
		
		builder.bind_pipeline_graphics(self.pipeline.clone())
		       .bind_vertex_buffers(0, self.vertices.clone())
		       .bind_any_index_buffer(self.indices.clone())
		       .bind_descriptor_sets(PipelineBindPoint::Graphics,
		                             self.pipeline.layout().clone(),
		                             0,
		                             self.set.clone())
		       .push_constants(self.pipeline.layout().clone(),
		                       0,
		                       (transform.to_homogeneous(), color))
		       .draw_indexed(self.indices.len() as u32,
		                     1,
		                     0,
		                     0,
		                     0)?;
		
		Ok(())
	}
}

impl Component for SimpleModel {
	fn render(&self, entity: &Entity, _renderer: &Renderer, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		self.render_impl(Similarity3::from_isometry(*entity.state().position, 1.0), Color::full_white(), builder)?;
		
		Ok(())
	}
}


