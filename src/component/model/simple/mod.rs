use std::sync::Arc;
use vulkano::buffer::{Buffer, Subbuffer, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint};
use vulkano::sync::GpuFuture;

pub mod asset;

pub use crate::renderer::pipelines::default::Vertex;
use crate::application::Entity;
use crate::component::{Component, ComponentBase, ComponentError, ComponentInner};
use crate::math::{AABB, aabb_from_points, Color, Point3, Similarity3};
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::renderer::assets_manager::TextureBundle;
use crate::renderer::pipelines::default::{DefaultPipeline, Pc};
use crate::utils::{AutoCommandBufferBuilderEx, BufferEx, IntoInfo, FenceCheck, IndexSubbuffer};
use super::{ModelError, VertexIndex};
pub use asset::{ObjAsset, ObjLoadError};


#[derive(ComponentBase, Clone)]
pub struct SimpleModel {
	#[inner] inner: ComponentInner,
	aabb: AABB,
	pipeline: Arc<GraphicsPipeline>,
	pub vertices: Subbuffer<[Vertex]>,
	pub indices: IndexSubbuffer,
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
		
		let mut upload_buffer = AutoCommandBufferBuilder::primary(&*renderer.command_buffer_allocator,
		                                                          renderer.load_queue.queue_family_index(),
		                                                          CommandBufferUsage::OneTimeSubmit)?;
		
		let vertices = Buffer::upload_iter(&renderer.memory_allocator,
		                                   BufferUsage::VERTEX_BUFFER.into_info(),
		                                   vertices.iter().cloned(),
		                                   &mut upload_buffer)?;
		
		let indices = Buffer::upload_iter(&renderer.memory_allocator,
		                                  BufferUsage::INDEX_BUFFER.into_info(),
		                                  indices.iter().copied(),
		                                  &mut upload_buffer)?;
		
		let set = PersistentDescriptorSet::new(&renderer.descriptor_set_allocator,
		                                       pipeline.layout().set_layouts().get(0).ok_or(ModelError::NoLayout)?.clone(), [
			                                       WriteDescriptorSet::buffer(0, renderer.commons.clone()),
			                                       WriteDescriptorSet::image_view_sampler(1, texture.image.clone(), texture.sampler.clone()),
		                                       ])?;
		
		let upload_future = upload_buffer.build()?
		                                 .execute(renderer.load_queue.clone())?;
		
		let fence = FenceCheck::new(upload_future.join(texture.fence.future()))?;
		
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
	
	pub fn render_impl(&self, transform: Similarity3, color: Color, context: &mut RenderContext) -> Result<(), ComponentError> {
		if !self.loaded() { return Ok(()) }
		
		context.builder.bind_pipeline_graphics(self.pipeline.clone())
		               .bind_vertex_buffers(0, self.vertices.clone())
		               .bind_any_index_buffer(self.indices.clone())
		               .bind_descriptor_sets(PipelineBindPoint::Graphics,
		                                     self.pipeline.layout().clone(),
		                                     0,
		                                     self.set.clone())
		               .push_constants(self.pipeline.layout().clone(),
		                               0,
		                               Pc {
			                               model: transform.to_homogeneous().into(),
			                               color: color.into(),
		                               })
		               .draw_indexed(self.indices.len() as u32,
		                             1,
		                             0,
		                             0,
		                             0)?;
		
		Ok(())
	}
}

impl Component for SimpleModel {
	fn render(&self, entity: &Entity, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<(), ComponentError> {
		let base_pos = *entity.state().position;
		
		self.render_impl(Similarity3::from_isometry(base_pos, 1.0), Color::full_white(), context)?;
		
		Ok(())
	}
}


