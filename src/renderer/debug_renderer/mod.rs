use std::cell::{RefCell, RefMut};
use std::sync::Arc;
use err_derive::Error;
use nalgebra::Unit;
use vulkano::command_buffer;
use vulkano::buffer::BufferUsage;
use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::descriptor_set::PersistentDescriptorSet;
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::Queue;
use vulkano::memory::allocator::{MemoryUsage, StandardMemoryAllocator};
use vulkano::pipeline::{Pipeline, GraphicsPipeline, PipelineBindPoint};

mod text_cache;

use crate::component::model::SimpleModel;
use crate::component::model::simple::asset::{ObjAsset, ObjLoadError};
use crate::debug::{DEBUG_POINTS, DebugPoint, DEBUG_LINES, DebugLine, DEBUG_TEXTS, DebugText, DEBUG_BOXES, DEBUG_CAPSULES, DEBUG_SPHERES, DebugBox, DebugSphere, DebugCapsule};
use crate::math::{Vec2, Rot2, PMat4, Isometry3, Similarity3, face_upwards_lossy, PI};
use crate::utils::{AutoCommandBufferBuilderEx, SubbufferAllocatorEx, SubbufferAllocatorExError};
use super::{Renderer, RenderContext};
use super::assets_manager::TextureAsset;
use super::pipelines::{Pipelines, PipelineError};
use super::pipelines::debug::{DebugPipeline, DebugTexturedPipeline, DebugShapePipeline, ShapePc, Vertex, TexturedVertex};
pub use text_cache::{TextCache, TextCacheError, TextCacheGetError};


pub struct DebugRenderer {
	text_cache: RefCell<TextCache>,
	pipeline: Arc<GraphicsPipeline>,
	text_pipeline: Arc<GraphicsPipeline>,
	shape_pipeline: Arc<GraphicsPipeline>,
	vertices_allocator: SubbufferAllocator,
	indexes_allocator: SubbufferAllocator,
	vertices: Vec<Vertex>,
	text_vertices: Vec<TexturedVertex>,
	indexes: Vec<u32>,
	models: Option<DebugModels>,
}

pub struct DebugModels {
	dbox: SimpleModel,
	sphere: SimpleModel,
	cbody: SimpleModel,
	ccap: SimpleModel,
}

const RING_MIN: f32 = 5.0;
const RING_WIDTH: f32 = 0.9;

impl DebugRenderer {
	pub fn new(queue: &Arc<Queue>, memory_allocator: &Arc<StandardMemoryAllocator>, command_buffer_allocator: &Arc<StandardCommandBufferAllocator>, descriptor_set_allocator: &Arc<StandardDescriptorSetAllocator>, pipelines: &mut Pipelines) -> Result<DebugRenderer, DebugRendererError> {
		let pipeline = pipelines.get::<DebugPipeline>()?;
		let text_pipeline = pipelines.get::<DebugTexturedPipeline>()?;
		let shape_pipeline = pipelines.get::<DebugShapePipeline>()?;
		
		let vertices_allocator = SubbufferAllocator::new(memory_allocator.clone(),
		                                                 SubbufferAllocatorCreateInfo {
			                                                 memory_usage: MemoryUsage::Upload,
			                                                 buffer_usage: BufferUsage::VERTEX_BUFFER,
			                                                 ..SubbufferAllocatorCreateInfo::default()
		                                                 });
		
		let indexes_allocator = SubbufferAllocator::new(memory_allocator.clone(),
		                                                SubbufferAllocatorCreateInfo {
			                                                memory_usage: MemoryUsage::Upload,
			                                                buffer_usage: BufferUsage::INDEX_BUFFER,
			                                                ..SubbufferAllocatorCreateInfo::default()
		                                                });
		
		let text_cache = RefCell::new(TextCache::new(queue, memory_allocator, command_buffer_allocator, descriptor_set_allocator, pipelines)?);
		
		Ok(DebugRenderer {
			pipeline,
			text_pipeline,
			shape_pipeline,
			vertices_allocator,
			indexes_allocator,
			vertices: vec![],
			text_vertices: vec![],
			indexes: vec![],
			text_cache,
			models: None,
		})
	}
	
	pub fn text_cache(&self) -> RefMut<TextCache> {
		self.text_cache.borrow_mut()
	}
	
	pub fn before_render(&mut self, renderer: &mut Renderer) -> Result<(), DebugRendererPreRenderError> {
		if self.models.is_none() {
			let mut load_models = false;
			DEBUG_BOXES.with(|boxes| load_models |= !boxes.borrow().is_empty());
			DEBUG_SPHERES.with(|spheres| load_models |= !spheres.borrow().is_empty());
			DEBUG_CAPSULES.with(|capsules| load_models |= !capsules.borrow().is_empty());
			
			if load_models {
				self.models = Some(DebugModels {
					dbox: renderer.load(ObjAsset::at("debug/box.obj", TextureAsset::at("debug/tex.png").nearest().no_mipmaps()))?,
					sphere: renderer.load(ObjAsset::at("debug/sphere.obj", TextureAsset::at("debug/tex.png").nearest().no_mipmaps()))?,
					cbody: renderer.load(ObjAsset::at("debug/cbody.obj", TextureAsset::at("debug/tex.png").nearest().no_mipmaps()))?,
					ccap: renderer.load(ObjAsset::at("debug/ccap.obj", TextureAsset::at("debug/tex.png").nearest().no_mipmaps()))?,
				});
			}
		}
		
		Ok(())
	}
	
	pub fn render(&mut self, context: &mut RenderContext) -> Result<(), DebugRendererRenderError> {
		let viewproj = (
			context.projection.0 * context.view.0,
			context.projection.1 * context.view.1,
		);
		
		DEBUG_LINES.with(|lines| {
			for line in lines.borrow_mut().iter() {
				if line.width <= 0.0 {
					continue;
				} else {
					self.draw_line(line, &viewproj, &context.pixel_scale);
				}
			}
		});
		
		DEBUG_POINTS.with(|points| {
			for point in points.borrow_mut().iter() {
				if point.radius <= 0.0 {
					continue;
				} else if point.radius <= RING_MIN {
					self.draw_circle(point, &viewproj, &context.pixel_scale);
				} else {
					self.draw_ring(point, &viewproj, &context.pixel_scale);
				}
			}
		});
		
		if !self.vertices.is_empty() {
			let vertex_buffer = self.vertices_allocator.from_iter(self.vertices.drain(..))?;
			let index_buffer = self.indexes_allocator.from_iter(self.indexes.drain(..))?;
			let index_count = index_buffer.len();
			
			context.builder.bind_pipeline_graphics(self.pipeline.clone())
			               .bind_index_buffer(index_buffer)
			               .bind_vertex_buffers(0, vertex_buffer)
			               .draw_indexed(index_count as u32,
			                             1,
			                             0,
			                             0,
			                             0)?;
		}
		
		DEBUG_TEXTS.with(|texts| {
			let texts = texts.borrow();
			
			if !texts.is_empty() {
				context.builder.bind_pipeline_graphics(self.text_pipeline.clone());
			}
			
			for text in texts.iter() {
				if text.size <= 0.0 || text.text.is_empty() {
					continue;
				} else if let Some(set) = self.draw_text(text, &viewproj, &context.pixel_scale)? {
					let vertex_buffer = self.vertices_allocator.from_iter(self.text_vertices.drain(..))?;
					let index_buffer = self.indexes_allocator.from_iter(self.indexes.drain(..))?;
					let index_count = index_buffer.len();
					
					context.builder.bind_index_buffer(index_buffer)
					               .bind_vertex_buffers(0, vertex_buffer)
					               .bind_descriptor_sets(PipelineBindPoint::Graphics,
					                                     self.text_pipeline.layout().clone(),
					                                     0,
					                                     set)
					               .draw_indexed(index_count as u32,
					                             1,
					                             0,
					                             0,
					                             0)?;
				}
			}
			
			Ok::<_, DebugRendererRenderError>(())
		})?;
		
		DEBUG_CAPSULES.with(|capsules| self.draw_capsules(&mut *capsules.borrow_mut(), context.builder))?;
		DEBUG_BOXES.with(|boxes| self.draw_boxes(&mut *boxes.borrow_mut(), context.builder))?;
		DEBUG_SPHERES.with(|spheres| self.draw_spheres(&mut *spheres.borrow_mut(), context.builder))?;
		
		Ok(())
	}
	
	pub fn reset(&mut self) {
		DEBUG_POINTS.with(|points| points.borrow_mut().clear());
		DEBUG_LINES.with(|lines| lines.borrow_mut().clear());
		DEBUG_TEXTS.with(|texts| texts.borrow_mut().clear());
		DEBUG_CAPSULES.with(|capsules| capsules.borrow_mut().clear());
		DEBUG_BOXES.with(|boxes| boxes.borrow_mut().clear());
		DEBUG_SPHERES.with(|spheres| spheres.borrow_mut().clear());
		
		self.text_cache.borrow_mut().cleanup();
	}
	
	fn draw_circle(&mut self, point: &DebugPoint, viewproj: &(PMat4, PMat4), pixel_scale: &Vec2) {
		let edges = point.radius.log(1.2).max(4.0) as u32;
		let center = point.position.project(viewproj);
		
		let mut last_ids = (self.vertices.len() as u32, self.vertices.len() as u32 + edges - 1);
		let mut sub = true;
		
		while last_ids.0 + 1 < last_ids.1 {
			self.indexes.push(last_ids.0);
			self.indexes.push(last_ids.1);
			
			if sub {
				last_ids.0 += 1;
				self.indexes.push(last_ids.0);
			} else {
				last_ids.1 -= 1;
				self.indexes.push(last_ids.1);
			}
			
			sub = !sub
		}
		
		for id in 0..edges {
			let angle = PI * 2.0 / edges as f32 * id as f32;
			let offset = Rot2::new(angle).transform_vector(&Vec2::x()).component_mul(pixel_scale) * point.radius;
			self.vertices.push(Vertex::new(
				&center.0.coords + offset.to_homogeneous(),
				&center.1.coords + offset.to_homogeneous(),
				&point.color,
			));
		}
	}
	
	fn draw_ring(&mut self, point: &DebugPoint, viewproj: &(PMat4, PMat4), pixel_scale: &Vec2) {
		let edges = (point.radius.ln() * 9.0).max(4.0).min(128.0) as u32;
		let center = point.position.project(viewproj);
		
		let start = self.vertices.len() as u32;
		self.indexes.push(start);
		self.indexes.push(start + edges * 2 - 2);
		self.indexes.push(start + edges * 2 - 1);
		self.indexes.push(start);
		self.indexes.push(start + edges * 2 - 1);
		self.indexes.push(start + 1);
		for id in 0..(edges - 1) {
			self.indexes.push(start + id * 2);
			self.indexes.push(start + id * 2 + 1);
			self.indexes.push(start + id * 2 + 2);
			self.indexes.push(start + id * 2 + 1);
			self.indexes.push(start + id * 2 + 3);
			self.indexes.push(start + id * 2 + 2);
		}
		
		for id in 0..edges {
			let angle = PI * 2.0 / edges as f32 * id as f32;
			let dir = Rot2::new(angle).transform_vector(&Vec2::x()).component_mul(&pixel_scale);
			let offset: Vec2 = &dir * point.radius;
			let offset_inner: Vec2 = &dir * ((point.radius - RING_MIN / 2.0) * RING_WIDTH);
			self.vertices.push(Vertex::new(
				&center.0 + offset.to_homogeneous(),
				&center.1 + offset.to_homogeneous(),
				&point.color,
			));
			self.vertices.push(Vertex::new(
				&center.0 + offset_inner.to_homogeneous(),
				&center.1 + offset_inner.to_homogeneous(),
				&point.color,
			));
		}
	}
	
	fn draw_line(&mut self, line: &DebugLine, viewproj: &(PMat4, PMat4), pixel_scale: &Vec2) {
		let edges = (line.width.ln() * 4.5).max(2.0) as u32;
		let from = line.from.project(viewproj);
		let to = line.to.project(viewproj);
		
		if from.0.z < 0.0 || to.0.z < 0.0 || from.0.z > 1.0 || to.0.z > 1.0
		|| from.1.z < 0.0 || to.1.z < 0.0 || from.1.z > 1.0 || to.1.z > 1.0 {
			return
		}
		
		let dir = Unit::try_new((from.0 - to.0).xy(), std::f32::EPSILON).unwrap_or(Vec2::x_axis());
		let dir = Rot2::rotation_between_axis(&Vec2::x_axis(), &dir);
		
		let base_id = self.vertices.len() as u32;
		let mut last_ids = (edges / 2 - 1, edges / 2);
		let mut sub = false;
		
		while last_ids.0 != last_ids.1 + 1 {
			self.indexes.push(base_id + last_ids.1);
			self.indexes.push(base_id + last_ids.0);
			
			if sub {
				if last_ids.0 == 0 {
					last_ids.0 = edges * 2 - 1;
				} else {
					last_ids.0 = last_ids.0 - 1;
				}
				self.indexes.push(base_id + last_ids.0);
			} else {
				last_ids.1 += 1;
				self.indexes.push(base_id + last_ids.1);
			}
			
			sub = !sub
		}
		
		for id in 0..edges {
			let angle = dir * Rot2::from_angle(PI / edges as f32 * id as f32 - PI / 2.0);
			let offset = angle.transform_vector(&Vec2::x()).component_mul(pixel_scale) * line.width;
			self.vertices.push(Vertex::new(
				&from.0 + offset.to_homogeneous(),
				&from.1 + offset.to_homogeneous(),
				&line.color
			));
		}
	
		for id in 0..edges {
			let angle = dir * Rot2::from_angle(PI / edges as f32 * id as f32 - PI / 2.0);
			let offset = angle.transform_vector(&-Vec2::x()).component_mul(pixel_scale) * line.width;
			self.vertices.push(Vertex::new(
				&to.0 + offset.to_homogeneous(),
				&to.1 + offset.to_homogeneous(),
				&line.color
			));
		}
	}
	
	fn draw_text(&mut self, text: &DebugText, viewproj: &(PMat4, PMat4), pixel_scale: &Vec2) -> Result<Option<Arc<PersistentDescriptorSet>>, DebugRendererRenderError> {
		let entry = self.text_cache.get_mut().get(&text.text)?;
		
		let size_px = vector!(entry.size.0 as f32 / entry.size.1 as f32 * text.size, text.size);
		let offset = text.offset.evaluate(size_px).coords.component_mul(&pixel_scale).to_homogeneous();
		let top_left = text.position.project(viewproj);
		let top_left = (
			top_left.0 + &offset,
			top_left.1 + &offset,
		);
		let size = size_px.component_mul(&pixel_scale);
		
		let start_id = self.text_vertices.len() as u32;
		self.indexes.push(start_id);
		self.indexes.push(start_id + 2);
		self.indexes.push(start_id + 1);
		self.indexes.push(start_id);
		self.indexes.push(start_id + 3);
		self.indexes.push(start_id + 2);
		
		self.text_vertices.push(TexturedVertex::new(
			top_left.0,
			top_left.1,
			[0.0, 0.0],
			&text.color,
		));
		self.text_vertices.push(TexturedVertex::new(
			top_left.0 + vector!(size.x, 0.0, 0.0),
			top_left.1 + vector!(size.x, 0.0, 0.0),
			[1.0, 0.0],
			&text.color,
		));
		self.text_vertices.push(TexturedVertex::new(
			top_left.0 + vector!(size.x, size.y, 0.0),
			top_left.1 + vector!(size.x, size.y, 0.0),
			[1.0, 1.0],
			&text.color,
		));
		self.text_vertices.push(TexturedVertex::new(
			top_left.0 + vector!(0.0, size.y, 0.0),
			top_left.1 + vector!(0.0, size.y, 0.0),
			[0.0, 1.0],
			&text.color,
		));
		
		Ok(Some(entry.set.clone()))
	}
	
	fn draw_boxes(&self, boxes: &mut Vec<DebugBox>, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), DebugRendererRenderError> {
		let models = match self.models.as_ref() {
			Some(models) if models.dbox.loaded() => { models }
			_ => {
				return Ok(());
			}
		};
		
		if !boxes.is_empty() {
			builder.bind_pipeline_graphics(self.shape_pipeline.clone())
			       .bind_vertex_buffers(0, models.dbox.vertices.clone())
			       .bind_any_index_buffer(models.dbox.indices.clone())
			       .bind_descriptor_sets(PipelineBindPoint::Graphics,
			                             self.shape_pipeline.layout().clone(),
			                             0,
			                             models.dbox.set.clone());
		}
		
		for dbox in boxes.iter() {
			let transform = dbox.position.to_homogeneous().prepend_nonuniform_scaling(&dbox.size);
			
			builder.push_constants(self.shape_pipeline.layout().clone(),
			                       0,
			                       ShapePc {
				                       model: transform.into(),
				                       color: dbox.color.into(),
				                       edge: dbox.edge.into(),
			                       })
			       .draw_indexed(models.dbox.indices.len() as u32,
			                     1,
			                     0,
			                     0,
			                     0)?;
		}
		
		Ok(())
	}
	
	fn draw_spheres(&self, spheres: &mut Vec<DebugSphere>, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), DebugRendererRenderError> {
		let models = match self.models.as_ref() {
			Some(models) if models.sphere.loaded() => { models }
			_ => {
				return Ok(());
			}
		};
		
		if !spheres.is_empty() {
			builder.bind_pipeline_graphics(self.shape_pipeline.clone())
			       .bind_vertex_buffers(0, models.sphere.vertices.clone())
			       .bind_any_index_buffer(models.sphere.indices.clone())
			       .bind_descriptor_sets(PipelineBindPoint::Graphics,
			                             self.shape_pipeline.layout().clone(),
			                             0,
			                             models.sphere.set.clone());
		}
		
		for sphere in spheres.iter() {
			let transform = Similarity3::from_isometry(sphere.position, sphere.radius * 2.0).to_homogeneous();
			
			builder.push_constants(self.shape_pipeline.layout().clone(),
			                       0,
			                       ShapePc {
				                       model: transform.into(),
				                       color: sphere.color.into(),
				                       edge: sphere.edge.into(),
			                       })
			       .draw_indexed(models.sphere.indices.len() as u32,
			                     1,
			                     0,
			                     0,
			                     0)?;
		}
		
		Ok(())
	}
	
	fn draw_capsules(&self, capsules: &mut Vec<DebugCapsule>, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), DebugRendererRenderError> {
		let models = match self.models.as_ref() {
			Some(models) if models.ccap.loaded()
			             && models.cbody.loaded() => { models }
			_ => {
				return Ok(());
			}
		};
		
		if !capsules.is_empty() {
			builder.bind_pipeline_graphics(self.shape_pipeline.clone())
			       .bind_vertex_buffers(0, models.ccap.vertices.clone())
			       .bind_any_index_buffer(models.ccap.indices.clone())
			       .bind_descriptor_sets(PipelineBindPoint::Graphics,
			                             self.shape_pipeline.layout().clone(),
			                             0,
			                             models.ccap.set.clone());
		}
		
		for capsule in capsules.iter() {
			let dir = capsule.point_b - capsule.point_a;
			let transform_a = Similarity3::from_parts(capsule.point_a.coords.into(), face_upwards_lossy(-dir), capsule.radius * 2.0).to_homogeneous();
			let transform_b = Similarity3::from_parts(capsule.point_b.coords.into(), face_upwards_lossy(dir), capsule.radius * 2.0).to_homogeneous();
			
			builder.push_constants(self.shape_pipeline.layout().clone(),
			                       0,
			                       ShapePc {
				                       model: transform_a.into(),
				                       color: capsule.color.into(),
				                       edge: capsule.edge.into(),
			                       })
			       .draw_indexed(models.ccap.indices.len() as u32,
			                     1,
			                     0,
			                     0,
			                     0)?
			       .push_constants(self.shape_pipeline.layout().clone(),
			                       0,
			                       ShapePc {
				                       model: transform_b.into(),
				                       color: capsule.color.into(),
				                       edge: capsule.edge.into(),
			                       })
			       .draw_indexed(models.ccap.indices.len() as u32,
			                     1,
			                     0,
			                     0,
			                     0)?;
		}
		
		if !capsules.is_empty() {
			builder.bind_pipeline_graphics(self.shape_pipeline.clone())
			       .bind_vertex_buffers(0, models.cbody.vertices.clone())
			       .bind_any_index_buffer(models.cbody.indices.clone())
			       .bind_descriptor_sets(PipelineBindPoint::Graphics,
			                             self.shape_pipeline.layout().clone(),
			                             0,
			                             models.cbody.set.clone());
		}
		
		for capsule in capsules.iter() {
			let dir = capsule.point_b - capsule.point_a;
			let transform = Isometry3::from_parts((capsule.point_a.coords + dir / 2.0).into(), face_upwards_lossy(dir))
			                          .to_homogeneous()
			                          .prepend_nonuniform_scaling(&vector!(capsule.radius * 2.0, dir.magnitude(), capsule.radius * 2.0));
			
			builder.push_constants(self.shape_pipeline.layout().clone(),
			                       0,
			                       ShapePc {
				                       model: transform.into(),
				                       color: capsule.color.into(),
				                       edge: capsule.edge.into(),
			                       })
			       .draw_indexed(models.cbody.indices.len() as u32,
			                     1,
			                     0,
			                     0,
			                     0)?;
		}
		
		Ok(())
	}
}


#[derive(Debug, Error)]
pub enum DebugRendererError {
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] TextCacheError(#[error(source)] TextCacheError),
}

#[derive(Debug, Error)]
pub enum DebugRendererRenderError {
	#[error(display = "{}", _0)] TextCacheGetError(#[error(source)] TextCacheGetError),
	#[error(display = "{}", _0)] SubbufferAllocatorExError(#[error(source)] SubbufferAllocatorExError),
	#[error(display = "{}", _0)] DrawIndexedError(#[error(source)] command_buffer::PipelineExecutionError),
}

#[derive(Debug, Error)]
pub enum DebugRendererPreRenderError {
	#[error(display = "{}", _0)] ObjLoadError(#[error(source)] ObjLoadError),
}
