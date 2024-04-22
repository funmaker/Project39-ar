use std::sync::Arc;
use egui::Vec2;
use anyhow::Result;
use vulkano::buffer::{Buffer, Subbuffer, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::ImageAccess;
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint};
use vulkano::sync::GpuFuture;

mod pipeline;

use crate::application::Entity;
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::renderer::assets_manager::TextureAsset;
use crate::renderer::pipelines::PipelineNoLayoutError;
use crate::utils::{BufferEx, IntoInfo, FenceCheck};
use super::{Component, ComponentBase, ComponentInner};
use pipeline::{SrgbTestPipeline, Vertex, Pc};


#[derive(ComponentBase)]
pub struct SrgbTest {
	#[inner] inner: ComponentInner,
	image_size: Vec2,
	pipeline: Arc<GraphicsPipeline>,
	vertices: Subbuffer<[Vertex]>,
	set: Arc<PersistentDescriptorSet>,
	fence: FenceCheck,
}

impl SrgbTest {
	pub fn new(renderer: &mut Renderer) -> Result<Self> {
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
		
		let mut upload_buffer = AutoCommandBufferBuilder::primary(&*renderer.command_buffer_allocator,
		                                                          renderer.load_queue.queue_family_index(),
		                                                          CommandBufferUsage::OneTimeSubmit)?;
		
		let vertices = Buffer::upload_iter(&renderer.memory_allocator,
		                                   BufferUsage::VERTEX_BUFFER.into_info(),
		                                   square.iter().cloned(),
		                                   &mut upload_buffer)?;
		
		let set = PersistentDescriptorSet::new(&renderer.descriptor_set_allocator,
		                                       pipeline.layout().set_layouts().get(0).ok_or(PipelineNoLayoutError)?.clone(), [
			                                       WriteDescriptorSet::image_view_sampler(0, pattern.image, pattern.sampler),
		                                       ])?;
		
		let upload_future = upload_buffer.build()?
		                                 .execute(renderer.load_queue.clone())?;
		
		let fence = FenceCheck::new(upload_future.join(pattern.fence.future()))?;
		
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
	fn render(&self, _entity: &Entity, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<()> {
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
		                               Pc {
			                               scale: scale.into(),
		                               })
		               .draw(self.vertices.len() as u32,
		                     1,
		                     0,
		                     0)?;
		
		Ok(())
	}
}
