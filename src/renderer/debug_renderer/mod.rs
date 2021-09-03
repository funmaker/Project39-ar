use std::sync::Arc;
use std::f32::consts::PI;
use err_derive::Error;
use vulkano::{memory, command_buffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::buffer::{CpuBufferPool, BufferUsage, TypedBufferAccess};
use vulkano::device::Queue;
use vulkano::descriptor_set::DescriptorSet;
use vulkano::pipeline::{GraphicsPipeline, PipelineBindPoint};
use nalgebra::Unit;

mod text_cache;

use crate::debug::{DEBUG_POINTS, DebugPoint, DEBUG_LINES, DebugLine, DEBUG_TEXTS, DebugText};
use crate::math::{Vec2, Vec3, Rot2, PMat4};
use super::pipelines::debug::{DebugPipeline, DebugTexturedPipeline, Vertex, TexturedVertex};
use super::pipelines::{Pipelines, PipelineError};
use super::CommonsUBO;
use text_cache::{TextCache, TextCacheError, TextCacheGetError};

pub struct DebugRenderer {
	pipeline: Arc<GraphicsPipeline>,
	text_pipeline: Arc<GraphicsPipeline>,
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
	pub fn new(load_queue: &Arc<Queue>, pipelines: &mut Pipelines) -> Result<DebugRenderer, DebugRendererError> {
		let device = load_queue.device();
		let pipeline = pipelines.get::<DebugPipeline>()?;
		let text_pipeline = pipelines.get::<DebugTexturedPipeline>()?;
		
		let vertices_pool = CpuBufferPool::new(device.clone(), BufferUsage::vertex_buffer());
		let text_vertices_pool = CpuBufferPool::new(device.clone(), BufferUsage::vertex_buffer());
		let indexes_pool = CpuBufferPool::new(device.clone(), BufferUsage::index_buffer());
		
		let text_cache = TextCache::new(load_queue, pipelines)?;
		
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
	
	pub fn render(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, commons: &CommonsUBO, pixel_scale: Vec2) -> Result<(), DebugRendererRenderError> {
		let viewproj = (
			commons.projection[0] * commons.view[0],
			commons.projection[1] * commons.view[1],
		);
		
		
		DEBUG_LINES.with(|lines| {
			for line in lines.borrow_mut().drain(..) {
				if line.width <= 0.0 {
					continue;
				} else {
					self.draw_line(line, &viewproj, &pixel_scale);
				}
			}
		});
		
		DEBUG_POINTS.with(|points| {
			for point in points.borrow_mut().drain(..) {
				if point.radius <= 0.0 {
					continue;
				} else if point.radius <= RING_MIN {
					self.draw_circle(point, &viewproj, &pixel_scale);
				} else {
					self.draw_ring(point, &viewproj, &pixel_scale);
				}
			}
		});
		
		if !self.vertices.is_empty() {
			let vertex_buffer = self.vertices_pool.chunk(self.vertices.drain(..))?;
			let index_buffer = self.indexes_pool.chunk(self.indexes.drain(..))?;
			let index_count = index_buffer.len();
			
			builder.bind_pipeline_graphics(self.pipeline.clone())
			       .bind_index_buffer(index_buffer)
			       .bind_vertex_buffers(0, vertex_buffer)
			       .draw_indexed(index_count as u32,
			                     1,
			                     0,
			                     0,
			                     0)?;
		}
		
		DEBUG_TEXTS.with(|texts| {
			for text in texts.borrow_mut().drain(..) {
				if text.size <= 0.0 || text.text.is_empty() {
					continue;
				} else if let Some(set) = self.draw_text(text, &viewproj, &pixel_scale)? {
					let vertex_buffer = self.text_vertices_pool.chunk(self.text_vertices.drain(..))?;
					let index_buffer = self.indexes_pool.chunk(self.indexes.drain(..))?;
					let index_count = index_buffer.len();
					
					builder.bind_pipeline_graphics(self.text_pipeline.clone())
					       .bind_index_buffer(index_buffer)
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
		
		Ok(())
	}
	
	fn draw_circle(&mut self, point: DebugPoint, viewproj: &(PMat4, PMat4), pixel_scale: &Vec2) {
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
			let angle = std::f32::consts::TAU / edges as f32 * id as f32;
			let offset = Rot2::new(angle).transform_vector(&Vec2::x()).component_mul(pixel_scale) * point.radius;
			self.vertices.push(Vertex::new(
				&center.0.coords + offset.to_homogeneous(),
				&center.1.coords + offset.to_homogeneous(),
				&point.color,
			));
		}
	}
	
	fn draw_ring(&mut self, point: DebugPoint, viewproj: &(PMat4, PMat4), pixel_scale: &Vec2) {
		let edges = (point.radius.ln() * 9.0).max(4.0) as u32;
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
			let angle = std::f32::consts::TAU / edges as f32 * id as f32;
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
	
	fn draw_line(&mut self, line: DebugLine, viewproj: &(PMat4, PMat4), pixel_scale: &Vec2) {
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
	
	fn draw_text(&mut self, text: DebugText, viewproj: &(PMat4, PMat4), pixel_scale: &Vec2) -> Result<Option<Arc<dyn DescriptorSet + Send + Sync>>, DebugRendererRenderError> {
		let entry = self.text_cache.get(&text.text)?;
		
		let size_px = Vec2::new(entry.size.0 as f32 / entry.size.1 as f32 * text.size, text.size);
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
			top_left.0 + Vec3::new(size.x, 0.0, 0.0),
			top_left.1 + Vec3::new(size.x, 0.0, 0.0),
			[1.0, 0.0],
			&text.color,
		));
		self.text_vertices.push(TexturedVertex::new(
			top_left.0 + Vec3::new(size.x, size.y, 0.0),
			top_left.1 + Vec3::new(size.x, size.y, 0.0),
			[1.0, 1.0],
			&text.color,
		));
		self.text_vertices.push(TexturedVertex::new(
			top_left.0 + Vec3::new(0.0, size.y, 0.0),
			top_left.1 + Vec3::new(0.0, size.y, 0.0),
			[0.0, 1.0],
			&text.color,
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
pub enum DebugRendererRenderError {
	#[error(display = "{}", _0)] TextCacheGetError(#[error(source)] TextCacheGetError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
	#[error(display = "{}", _0)] DrawIndexedError(#[error(source)] command_buffer::DrawIndexedError),
}
