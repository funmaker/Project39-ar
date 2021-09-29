use std::time::Duration;
use std::cell::{RefCell, Cell};
use std::sync::Arc;
use err_derive::Error;
use simba::scalar::SubsetOf;
use vulkano::buffer::{ImmutableBuffer, BufferUsage, TypedBufferAccess};
use vulkano::{sync, memory, descriptor_set};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::pipeline::{GraphicsPipeline, PipelineBindPoint};
use vulkano::descriptor_set::{DescriptorSet, PersistentDescriptorSet};

use crate::application::{Entity, Application};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError, ComponentRef};
use crate::math::{Isometry3, AMat4, Vec4, Vec3, Similarity3, Rot3};
use crate::component::parent::Parent;
use crate::renderer::Renderer;
use crate::utils::FenceCheck;
use crate::renderer::pipelines::toolgun_text::{Vertex, ToolGunTextPipeline};
use crate::renderer::pipelines::PipelineError;
use crate::component::tools::{Tool, get_all_tools};
use crate::application::input::Hand;

#[derive(ComponentBase)]
pub struct ToolGun {
	#[inner] inner: ComponentInner,
	parent: ComponentRef<Parent>,
	offset: Isometry3,
	pipeline: Arc<GraphicsPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	set: Arc<dyn DescriptorSet + Send + Sync>,
	fence: FenceCheck,
	scroll: Cell<f32>,
	tools: RefCell<Vec<Box<dyn Tool>>>,
	tool_id: usize,
}

impl ToolGun {
	pub fn new(offset: Isometry3, renderer: &mut Renderer) -> Result<Self, ToolGunError> {
		let pipeline = renderer.pipelines.get::<ToolGunTextPipeline>()?;
		
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
		
		let set = {
			let mut set_builder = PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layouts().get(0).ok_or(ToolGunError::NoLayout)?.clone());
			set_builder.add_buffer(renderer.commons.clone())?;
			Arc::new(set_builder.build()?)
		};
		
		let fence = FenceCheck::new(vertices_promise)?;
		
		Ok(ToolGun {
			inner: ComponentInner::new(),
			parent: ComponentRef::null(),
			offset,
			pipeline,
			vertices,
			set,
			fence,
			scroll: Cell::new(0.0),
			tools: RefCell::new(get_all_tools()),
			tool_id: 0,
		})
	}
}

impl Component for ToolGun {
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> {
		let state = entity.state();
		
		if self.parent.get(application).is_none() {
			let controller = application.find_entity(|e| e != entity && (e.name == "Controller" || e.name == "Hand") && (e.state().position.translation.vector - state.position.translation.vector).magnitude() < 0.1);
	
			if let Some(controller) = controller {
				self.parent.set(entity.add_component(Parent::new(controller, self.offset)));
				controller.state_mut().hidden = true;
			}
		}
		
		self.scroll.set(self.scroll.get() + delta_time.as_secs_f32());
		
		if application.input.fire(Hand::Right) {
		
		}
		
		Ok(())
	}
	
	fn render(&self, entity: &Entity, renderer: &Renderer, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		if !self.fence.check() { return Ok(()); }
		
		let tools = self.tools.borrow();
		let tool = tools.get(self.tool_id);
		let text = tool.map_or("None", |t| t.name());
		let text_entry = renderer.debug_text_cache().get(text)?;
		let text_pos = entity.state().position * Similarity3::from_parts(Vec3::new(0.000671, 0.059217, -0.027263).into(),
		                                                                 Rot3::from_euler_angles(0.781855066, 0.0, 0.0),
		                                                                 0.02135);
		let text_ratio = text_entry.size.0 as f32 / text_entry.size.1 as f32;
		let model_matrix: AMat4 = text_pos.to_superset();
		
		let uv_transform = Vec4::new(
			2.0 / text_ratio,
			2.0,
			(self.scroll.get() * 2.0) % (text_ratio + 2.0) / (text_ratio + 2.0) * (2.0 / text_ratio + 1.0) - (2.0 / text_ratio),
			-0.5
		);
		
		builder.bind_pipeline_graphics(self.pipeline.clone())
		       .bind_vertex_buffers(0, self.vertices.clone())
		       .bind_descriptor_sets(PipelineBindPoint::Graphics,
		                             self.pipeline.layout().clone(),
		                             0,
		                             (self.set.clone(), text_entry.set))
		       .push_constants(self.pipeline.layout().clone(),
		                       0,
		                       (model_matrix.to_homogeneous(), uv_transform))
		       .draw(self.vertices.len() as u32,
		             1,
		             0,
		             0)?;
		
		Ok(())
	}
	
	fn end(&self, _entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		if let Some(parent) = self.parent.entity().get(application) {
			parent.state_mut().hidden = false;
		}
		
		Ok(())
	}
}


#[derive(Debug, Error)]
pub enum ToolGunError {
	#[error(display = "Pipeline doesn't have specified layout")] NoLayout,
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
	#[error(display = "{}", _0)] DescriptorSetError(#[error(source)] descriptor_set::DescriptorSetError),
}
