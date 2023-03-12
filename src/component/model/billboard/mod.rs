use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;
use egui::Ui;
use vulkano::buffer::{BufferUsage, DeviceLocalBuffer, TypedBufferAccess};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::ImageAccess;
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint};

mod pipeline;

use crate::renderer::assets_manager::TextureAsset;
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::utils::{ExUi, FenceCheck};
use crate::math::{face_towards_lossy, Similarity3, to_euler, PI, Rot3};
use crate::component::{Component, ComponentBase, ComponentError, ComponentInner};
use crate::application::{Application, Entity};
use super::ModelError;
use pipeline::{FoodPipeline, Vertex, Pc};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract};
use vulkano::sync::GpuFuture;

#[derive(ComponentBase, Clone)]
pub struct Billboard {
	#[inner] inner: ComponentInner,
	ratio: f32,
	layers: u32,
	rotation: Cell<f32>,
	last_rot: Cell<Rot3>,
	pipeline: Arc<GraphicsPipeline>,
	vertices: Arc<DeviceLocalBuffer<[Vertex]>>,
	set: Arc<PersistentDescriptorSet>,
	fence: FenceCheck,
}

#[allow(dead_code)]
impl Billboard {
	pub fn new(texture: TextureAsset, renderer: &mut Renderer) -> Result<Billboard, ModelError> {
		let pipeline = renderer.pipelines.get::<FoodPipeline>()?;
		let texture = renderer.load(texture)?;
		let image_size = texture.image.image().dimensions().width_height();
		let ratio = image_size[0] as f32 / image_size[1] as f32;
		let layers = texture.image.image().dimensions().array_layers();
		
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
		
		let vertices = DeviceLocalBuffer::from_iter(&renderer.memory_allocator,
		                                            square.iter().cloned(),
		                                            BufferUsage{ vertex_buffer: true, ..BufferUsage::empty() },
		                                            &mut upload_buffer)?;
		
		let set = PersistentDescriptorSet::new(&renderer.descriptor_set_allocator,
		                                       pipeline.layout().set_layouts().get(0).ok_or(ModelError::NoLayout)?.clone(), [
			                                       WriteDescriptorSet::buffer(0, renderer.commons.clone()),
			                                       WriteDescriptorSet::image_view_sampler(1, texture.image.clone(), texture.sampler.clone()),
		                                       ])?;
		
		let upload_future = upload_buffer.build()?
		                                 .execute(renderer.load_queue.clone())?;
		
		let fence = FenceCheck::new(upload_future.join(texture.fence.future()))?;
		
		Ok(Billboard {
			inner: ComponentInner::from_render_type(RenderType::Transparent),
			ratio,
			layers,
			rotation: Cell::new(0.0),
			last_rot: Cell::new(Rot3::identity()),
			pipeline,
			vertices,
			set,
			fence,
		})
	}
}

impl Component for Billboard {
	fn tick(&self, entity: &Entity, _application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		let state = entity.state_mut();
		
		let relative = state.position.rotation / self.last_rot.get();
		let (_, yaw, _) = to_euler(relative);
		
		self.rotation.set(self.rotation.get() + yaw);
		self.last_rot.set(state.position.rotation);
		
		Ok(())
	}
	
	fn render(&self, entity: &Entity, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<(), ComponentError> {
		if !self.fence.check() { return Ok(()) }
		
		let position = entity.state().position.translation.vector;
		let to_camera = context.camera_pos.translation.vector - position;
		let rotation = face_towards_lossy(-to_camera);
		let transform = Similarity3::from_parts(position.into(), rotation, 0.05);
		let angle = self.rotation.get() - f32::atan2(to_camera.x, to_camera.z);
		let frame = (angle / PI / 2.0).rem_euclid(1.0) * self.layers as f32;
		
		context.builder.bind_pipeline_graphics(self.pipeline.clone())
		       .bind_vertex_buffers(0, self.vertices.clone())
		       .bind_descriptor_sets(PipelineBindPoint::Graphics,
		                             self.pipeline.layout().clone(),
		                             0,
		                             self.set.clone())
		       .push_constants(self.pipeline.layout().clone(),
		                       0,
		                       Pc {
			                       model: transform.to_homogeneous().into(),
			                       ratio: self.ratio,
			                       frame,
		                       })
		       .draw(self.vertices.len() as u32,
		             1,
		             0,
		             0)?;
		
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, _application: &Application) {
		ui.inspect_row("Ratio", format!("{}", self.ratio), ());
		ui.inspect_row("Frames", format!("{}", self.layers), ());
		ui.inspect_row("Rotation", &self.rotation, (0.1, 0.0..=PI*2.0));
	}
}


