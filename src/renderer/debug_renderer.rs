use std::sync::Arc;
use std::f32::consts::PI;
use err_derive::Error;
use cgmath::{Angle, Vector3, Vector2};
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState};
use vulkano::buffer::{CpuBufferPool, BufferUsage};
use vulkano::device::Device;
use vulkano::memory;

use crate::renderer::pipelines::debug::{DebugPipeline, Vertex};
use crate::renderer::pipelines::{Pipelines, PipelineError};
use crate::renderer::CommonsVBO;
use crate::debug::DEBUG_POINTS;

pub struct DebugRenderer {
	pipeline: Arc<DebugPipeline>,
	vertices_pool: CpuBufferPool<Vertex>,
	indexes_pool: CpuBufferPool<u32>,
	vertices: Vec<Vertex>,
	indexes: Vec<u32>,
}

impl DebugRenderer {
	pub fn new(device: &Arc<Device>, pipelines: &mut Pipelines) -> Result<DebugRenderer, DebugRendererError> {
		let pipeline = pipelines.get()?;
		
		let vertices_pool = CpuBufferPool::new(device.clone(), BufferUsage::vertex_buffer());
		let indexes_pool = CpuBufferPool::new(device.clone(), BufferUsage::index_buffer());
		
		Ok(DebugRenderer {
			pipeline,
			vertices_pool,
			indexes_pool,
			vertices: vec![],
			indexes: vec![],
		})
	}
	
	pub fn render(&mut self, builder: &mut AutoCommandBufferBuilder, commons: &CommonsVBO, pixel_scale: Vector2<f32>, eye: u32) -> Result<(), DebugRendererRederError> {
		let viewproj = commons.view[eye as usize] * commons.projection[eye as usize];
		
		DEBUG_POINTS.with(|points| {
			for point in points.borrow_mut().drain(..) {
				if point.radius <= 0.0 {
					continue;
				} else {
					let edges = (point.radius * 2.0).max(4.0) as u32;
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
						let offset = Vector2::new(angle.sin() * pixel_scale.x, angle.cos() * pixel_scale.y) * point.radius;
						self.vertices.push(Vertex::new(
							(center + offset.extend(1.0)).into(),
							point.color.into()
						));
					}
				}
			}
		});
		
		if self.vertices.is_empty() {
			return Ok(());
		}
		
		let vertex_buffer = self.vertices_pool.chunk(self.vertices.drain(..))?;
		let index_buffer = self.indexes_pool.chunk(self.indexes.drain(..))?;
		
		builder.draw_indexed(self.pipeline.clone(),
		                     &DynamicState::none(),
		                     vertex_buffer,
		                     index_buffer,
		                     (),
		                     ());
		
		Ok(())
	}
}


#[derive(Debug, Error)]
pub enum DebugRendererError {
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
}

#[derive(Debug, Error)]
pub enum DebugRendererRederError {
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
}
