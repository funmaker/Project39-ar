use std::sync::Arc;
use std::f32::consts::PI;
use err_derive::Error;
use cgmath::{Vector2, Matrix4, InnerSpace, Rad, Angle, ElementWise, Vector3};
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState};
use vulkano::buffer::{CpuBufferPool, BufferUsage};
use vulkano::device::Queue;
use vulkano::descriptor::DescriptorSet;
use vulkano::{memory, command_buffer};

mod text_cache;

use super::pipelines::debug::{DebugPipeline, DebugTexturedPipeline, Vertex, TexturedVertex};
use super::pipelines::{Pipelines, PipelineError};
use super::CommonsUBO;
use crate::debug::{DEBUG_POINTS, DebugPoint, DEBUG_LINES, DebugLine, DEBUG_TEXTS, DebugText};
use text_cache::{TextCache, TextCacheError, TextCacheGetError};

pub struct DebugRenderer {
	pipeline: Arc<DebugPipeline>,
	text_pipeline: Arc<DebugTexturedPipeline>,
	vertices_pool: CpuBufferPool<Vertex>,
	text_vertices_pool: CpuBufferPool<TexturedVertex>,
	indexes_pool: CpuBufferPool<u32>,
	vertices: Vec<Vertex>,
	text_vertices: Vec<TexturedVertex>,
	indexes: Vec<u32>,
	text_cache: TextCache,
}

const RING_MIN: f32 = 5.0;
const RING_WIDTH: f32 = 0.9;

impl DebugRenderer {
	pub fn new(queue: &Arc<Queue>, pipelines: &mut Pipelines) -> Result<DebugRenderer, DebugRendererError> {
		let device = queue.device();
		let pipeline = pipelines.get()?;
		let text_pipeline = pipelines.get()?;
		
		let vertices_pool = CpuBufferPool::new(device.clone(), BufferUsage::vertex_buffer());
		let text_vertices_pool = CpuBufferPool::new(device.clone(), BufferUsage::vertex_buffer());
		let indexes_pool = CpuBufferPool::new(device.clone(), BufferUsage::index_buffer());
		
		let text_cache = TextCache::new(queue, pipelines)?;
		
		Ok(DebugRenderer {
			pipeline,
			text_pipeline,
			vertices_pool,
			text_vertices_pool,
			indexes_pool,
			vertices: vec![],
			text_vertices: vec![],
			indexes: vec![],
			text_cache,
		})
	}
	
	pub fn render(&mut self, builder: &mut AutoCommandBufferBuilder, commons: &CommonsUBO, pixel_scale: Vector2<f32>, eye: u32) -> Result<(), DebugRendererRederError> {
		let viewproj = commons.projection[eye as usize] * commons.view[eye as usize];
		
		DEBUG_LINES.with(|lines| {
			for line in lines.borrow_mut().drain(..) {
				if line.width <= 0.0 {
					continue;
				} else {
					self.draw_line(line, viewproj, pixel_scale);
				}
			}
		});
		
		DEBUG_POINTS.with(|points| {
			for point in points.borrow_mut().drain(..) {
				if point.radius <= 0.0 {
					continue;
				} else if point.radius <= RING_MIN {
					self.draw_circle(point, viewproj, pixel_scale);
				} else {
					self.draw_ring(point, viewproj, pixel_scale);
				}
			}
		});
		
		if !self.vertices.is_empty() {
			let vertex_buffer = self.vertices_pool.chunk(self.vertices.drain(..))?;
			let index_buffer = self.indexes_pool.chunk(self.indexes.drain(..))?;
			
			builder.draw_indexed(self.pipeline.clone(),
			                     &DynamicState::none(),
			                     vertex_buffer,
			                     index_buffer,
			                     (),
			                     ())?;
		}
		
		DEBUG_TEXTS.with(|texts| {
			for text in texts.borrow_mut().drain(..) {
				if text.size <= 0.0 || text.text.is_empty() {
					continue;
				} else if let Some(set) = self.draw_text(text, viewproj, pixel_scale)? {
					let vertex_buffer = self.text_vertices_pool.chunk(self.text_vertices.drain(..))?;
					let index_buffer = self.indexes_pool.chunk(self.indexes.drain(..))?;
					
					builder.draw_indexed(self.text_pipeline.clone(),
					                     &DynamicState::none(),
					                     vertex_buffer,
					                     index_buffer,
					                     set,
					                     ())?;
				}
			}
			
			Ok::<_, DebugRendererRederError>(())
		})?;
		
		Ok(())
	}
	
	fn draw_circle(&mut self, point: DebugPoint, viewproj: Matrix4<f32>, pixel_scale: Vector2<f32>) {
		let edges = point.radius.log(1.2).max(4.0) as u32;
		let center = point.position.project(viewproj);
		
		let mut last_ids = (self.vertices.len() as u32, self.vertices.len() as u32 + edges - 1);
		let mut sub = true;
		
		while last_ids.0 + 1 < last_ids.1 {
			self.indexes.push(last_ids.1);
			self.indexes.push(last_ids.0);
			
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
			let angle = PI * 2.0 / edges as f32 * id as f32 + PI / 4.0;
			let offset = Vector2::new(angle.sin() * pixel_scale.x, angle.cos() * pixel_scale.y) * point.radius;
			self.vertices.push(Vertex::new(
				(center + offset.extend(0.0)).into(),
				point.color.into()
			));
		}
	}
	
	fn draw_ring(&mut self, point: DebugPoint, viewproj: Matrix4<f32>, pixel_scale: Vector2<f32>) {
		let edges = (point.radius.ln() * 9.0).max(4.0) as u32;
		let center = point.position.project(viewproj);
		
		let start = self.vertices.len() as u32;
		self.indexes.push(start + edges * 2 - 2);
		self.indexes.push(start);
		self.indexes.push(start + edges * 2 - 1);
		self.indexes.push(start + edges * 2 - 1);
		self.indexes.push(start);
		self.indexes.push(start + 1);
		for id in 0..(edges - 1) {
			self.indexes.push(start + id * 2);
			self.indexes.push(start + id * 2 + 2);
			self.indexes.push(start + id * 2 + 1);
			self.indexes.push(start + id * 2 + 1);
			self.indexes.push(start + id * 2 + 2);
			self.indexes.push(start + id * 2 + 3);
		}
		
		for id in 0..edges {
			let angle = PI * 2.0 / edges as f32 * id as f32 + PI / 4.0;
			let dir = Vector2::new(angle.sin() * pixel_scale.x, angle.cos() * pixel_scale.y);
			let offset = dir * point.radius;
			let offset_inner = dir * ((point.radius - RING_MIN / 2.0) * RING_WIDTH);
			self.vertices.push(Vertex::new(
				(center + offset.extend(0.0)).into(),
				point.color.into()
			));
			self.vertices.push(Vertex::new(
				(center + offset_inner.extend(0.0)).into(),
				point.color.into()
			));
		}
	}
	
	fn draw_line(&mut self, line: DebugLine, viewproj: Matrix4<f32>, pixel_scale: Vector2<f32>) {
		let edges = (line.width.ln() * 4.5).max(2.0) as u32;
		let from = line.from.project(viewproj);
		let to = line.to.project(viewproj);
		
		if from.z < 0.0 || to.z < 0.0 || from.z > 1.0 || to.z > 1.0 {
			return
		}
		
		let dir = (from - to).truncate().normalize();
		let mut dir = Rad::atan2(dir.x, dir.y).0;
		if !dir.is_normal() { dir = 0.0 }
		
		let base_id = self.vertices.len() as u32;
		let mut last_ids = (edges / 2 - 1, edges / 2);
		let mut sub = false;
		
		while last_ids.0 != last_ids.1 + 1 {
			self.indexes.push(base_id + last_ids.0);
			self.indexes.push(base_id + last_ids.1);
			
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
			let angle = dir + PI / edges as f32 * id as f32 - PI / 2.0;
			let offset = Vector2::new(angle.sin() * pixel_scale.x, angle.cos() * pixel_scale.y) * line.width;
			self.vertices.push(Vertex::new(
				(from + offset.extend(0.0)).into(),
				line.color.into()
			));
		}
	
		for id in 0..edges {
			let angle = dir + PI / edges as f32 * id as f32 + PI / 2.0;
			let offset = Vector2::new(angle.sin() * pixel_scale.x, angle.cos() * pixel_scale.y) * line.width;
			self.vertices.push(Vertex::new(
				(to + offset.extend(0.0)).into(),
				line.color.into()
			));
		}
	}
	
	fn draw_text(&mut self, text: DebugText, viewproj: Matrix4<f32>, pixel_scale: Vector2<f32>) -> Result<Option<Arc<dyn DescriptorSet + Send + Sync>>, DebugRendererRederError> {
		let entry = self.text_cache.get(&text.text)?;
		
		let size_px = Vector2::new(entry.size.0 as f32 / entry.size.1 as f32 * text.size, text.size);
		let top_left = text.position.project(viewproj) + text.offset.evaluate(size_px).mul_element_wise(pixel_scale).extend(0.0);
		let size = size_px.mul_element_wise(pixel_scale);
		
		let start_id = self.text_vertices.len() as u32;
		self.indexes.push(start_id);
		self.indexes.push(start_id + 1);
		self.indexes.push(start_id + 2);
		self.indexes.push(start_id);
		self.indexes.push(start_id + 2);
		self.indexes.push(start_id + 3);
		
		self.text_vertices.push(TexturedVertex::new(
			top_left.into(),
			[0.0, 0.0],
			text.color.into(),
		));
		self.text_vertices.push(TexturedVertex::new(
			(top_left + Vector3::new(size.x, 0.0, 0.0)).into(),
			[1.0, 0.0],
			text.color.into(),
		));
		self.text_vertices.push(TexturedVertex::new(
			(top_left + Vector3::new(size.x, size.y, 0.0)).into(),
			[1.0, 1.0],
			text.color.into(),
		));
		self.text_vertices.push(TexturedVertex::new(
			(top_left + Vector3::new(0.0, size.y, 0.0)).into(),
			[0.0, 1.0],
			text.color.into(),
		));
		
		Ok(Some(entry.set.clone()))
	}
}


#[derive(Debug, Error)]
pub enum DebugRendererError {
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] TextCacheError(#[error(source)] TextCacheError),
}

#[derive(Debug, Error)]
pub enum DebugRendererRederError {
	#[error(display = "{}", _0)] TextCacheGetError(#[error(source)] TextCacheGetError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
	#[error(display = "{}", _0)] DrawIndexedError(#[error(source)] command_buffer::DrawIndexedError),
}
