use std::sync::Arc;
use egui::Vec2;
use err_derive::Error;
use vulkano::{descriptor_set, memory, sync};
use vulkano::buffer::{BufferUsage, ImmutableBuffer, TypedBufferAccess};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::ImageAccess;
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint};
use vulkano::sync::GpuFuture;

mod pipeline;

use crate::application::Entity;
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::renderer::assets_manager::texture::{TextureAsset, TextureLoadError};
use crate::renderer::pipelines::PipelineError;
use crate::utils::FenceCheck;
use super::{Component, ComponentBase, ComponentInner, ComponentError};
use pipeline::{SrgbTestPipeline, Vertex};

#[derive(ComponentBase)]
pub struct SrgbTest {
	#[inner] inner: ComponentInner,
	image_size: Vec2,
	pipeline: Arc<GraphicsPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	set: Arc<PersistentDescriptorSet>,
	fence: FenceCheck,
}

impl SrgbTest {
	pub fn new(renderer: &mut Renderer) -> Result<Self, SrgbTestError> {
		let pipeline = renderer.pipelines.get::<SrgbTestPipeline>()?;
		
		let pattern = renderer.load(TextureAsset::at("cube/cube.png"))?;
		let image_size = pattern.image.image().dimensions().width_height();
		let image_size = Vec2::new(image_size[0] as f32, image_size[1] as f32);
		
		let square = [
			Vertex::new([-1.0, -1.0]),
			Vertex::new([-1.0,  1.0]),
			Vertex::new([ 1.0, -1.0]),
			Vertex::new([ 1.0, -1.0]),
			Vertex::new([-1.0,  1.0]),
			Vertex::new([ 1.0,  1.0]),
		];
		
		let (vertices, vertices_promise) = ImmutableBuffer::from_iter(square.iter().cloned(),
		                                                              BufferUsage{ vertex_buffer: true, ..BufferUsage::none() },
		                                                              renderer.queue.clone())?;
		
		let set = PersistentDescriptorSet::new(pipeline.layout().set_layouts().get(0).ok_or(SrgbTestError::NoLayout)?.clone(), [
			WriteDescriptorSet::image_view_sampler(0, pattern.image, pattern.sampler),
		])?;
		
		let fence = FenceCheck::new(vertices_promise.join(pattern.fence.future()))?;
		
		Ok(SrgbTest {
			inner: ComponentInner::from_render_type(RenderType::Opaque),
			image_size,
			pipeline,
			vertices,
			set,
			fence,
		})
	}
}

impl Component for SrgbTest {
	fn render(&self, _entity: &Entity, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<(), ComponentError> {
		if !self.fence.check() { return Ok(()); }
		
		let scale = self.image_size / Vec2::new(context.framebuffer_size.0 as f32, context.framebuffer_size.1 as f32);
		
		context.builder.bind_pipeline_graphics(self.pipeline.clone())
		               .bind_vertex_buffers(0, self.vertices.clone())
		               .bind_descriptor_sets(PipelineBindPoint::Graphics,
		                                     self.pipeline.layout().clone(),
		                                     0,
		                                     self.set.clone())
		               .push_constants(self.pipeline.layout().clone(),
		                               0,
		                               scale)
		               .draw(self.vertices.len() as u32,
		                     1,
		                     0,
		                     0)?;
		
		Ok(())
	}
}

#[derive(Debug, Error)]
pub enum SrgbTestError {
	#[error(display = "Pipeline doesn't have specified layout")] NoLayout,
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] TextureLoadError(#[error(source)] TextureLoadError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] DeviceMemoryAllocationError(#[error(source)] memory::DeviceMemoryAllocationError),
	#[error(display = "{}", _0)] DescriptorSetCreationError(#[error(source)] descriptor_set::DescriptorSetCreationError),
}
