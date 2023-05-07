use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;
use vulkano::buffer::{Buffer, BufferUsage, Subbuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::pipeline::{Pipeline, GraphicsPipeline, PipelineBindPoint};
use vulkano::sync::GpuFuture;

pub mod asset;
mod pipeline;

use crate::application::{Application, Entity, Hand};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::math::{Similarity3, Color, Point3, Isometry3, face_towards_lossy, Rot3, PI};
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::renderer::assets_manager::TextureBundle;
use crate::utils::{FenceCheck, IndexSubbuffer, AutoCommandBufferBuilderEx, BufferEx, IntoInfo};
use super::{ModelError, VertexIndex};
pub use asset::{GimpAsset, GimpLoadError};
pub use pipeline::Vertex;
use pipeline::{GimpPipeline, Pc};


#[derive(ComponentBase, Clone)]
pub struct GimpModel {
	#[inner] inner: ComponentInner,
	pipeline: Arc<GraphicsPipeline>,
	pub vertices: Subbuffer<[Vertex]>,
	pub indices: IndexSubbuffer,
	pub set: Arc<PersistentDescriptorSet>,
	pub fence: FenceCheck,
	active: Cell<bool>,
	time: Cell<f32>,
	orientation: Cell<Isometry3>,
}

#[allow(dead_code)]
impl GimpModel {
	pub fn new<VI>(vertices: &[Vertex],
	               indices: &[VI],
	               texture: TextureBundle,
	               normal_texture: TextureBundle,
	               renderer: &mut Renderer)
	               -> Result<GimpModel, ModelError>
	               where VI: VertexIndex {
		let pipeline = renderer.pipelines.get::<GimpPipeline>()?;
		
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
			                                       WriteDescriptorSet::image_view_sampler(2, normal_texture.image.clone(), normal_texture.sampler.clone()),
		                                       ])?;
		
		let upload_future = upload_buffer.build()?
		                                 .execute(renderer.load_queue.clone())?;
		
		let fence = FenceCheck::new(upload_future.join(texture.fence.future()))?;
		
		Ok(GimpModel {
			inner: ComponentInner::from_render_type(RenderType::Opaque),
			pipeline,
			vertices,
			indices: indices.into(),
			set,
			fence,
			active: Cell::new(false),
			time: Cell::new(0.0),
			orientation: Cell::new(Isometry3::identity()),
		})
	}
	
	pub fn loaded(&self) -> bool {
		self.fence.check()
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

impl Component for GimpModel {
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> {
		if application.input.fire_btn(Hand::Right).pressed {
			entity.unset_tag("Grabbed");
			self.active.set(true);
			self.time.set(entity.tag::<usize>("Id").unwrap_or(0) as f32 * -1.0);
			let camera = application.pov.get(application).unwrap();
			
			let mut orientation = *camera.state().position;
			let mut towards = orientation.rotation * vector!(0.0, 0.0, -1.0);
			towards.y = 0.0;
			orientation.rotation = face_towards_lossy(towards);
			self.orientation.set(orientation);
		}
		
		if self.active.get() {
			let time = self.time.get() + delta_time.as_secs_f32();
			self.time.set(time);
			
			let orientation = self.orientation.get();
			
			let mut state = entity.state_mut();
			if time >= 0.0 && time < 1.0 {
				let t = time;
				state.position.translation.vector = orientation.transform_point(&Point3::new( 0.4 - t * 0.5, 5.0 * t - 0.5 * 9.8 * t.powi(2) - 0.5, -0.3)).coords;
			} else if time >= 1.0 && time < 1.5 {
				let t = (time - 1.0) * 2.0;
				state.position.translation.vector = orientation.transform_point(&Point3::new(-0.25 + (t * PI).cos() * 0.15, -0.5 - (t * PI).sin() * 0.15, -0.3)).coords;
			} else if time >= 1.5 && time < 2.5 {
				let t = time - 1.5;
				state.position.translation.vector = orientation.transform_point(&Point3::new(-0.4 + t * 0.5, 5.0 * t - 0.5 * 9.8 * t.powi(2) - 0.5, -0.3)).coords;
			} else if time >= 2.5 && time < 3.0 {
				let t = (time - 2.5) * 2.0;
				state.position.translation.vector = orientation.transform_point(&Point3::new( 0.25 - (t * PI).cos() * 0.15, -0.5 - (t * PI).sin() * 0.15, -0.3)).coords;
			} else if time >= 3.0 {
				self.time.set(0.0);
			}
			state.position.rotation = Rot3::from_euler_angles(1.2, 0.4, 0.3).powf(time);
		}
		
		Ok(())
	}
	
	fn render(&self, entity: &Entity, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<(), ComponentError> {
		self.render_impl(Similarity3::from_isometry(*entity.state().position, 1.0), Color::full_white(), context)?;
		
		Ok(())
	}
}


