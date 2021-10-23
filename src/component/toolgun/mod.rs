use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::time::{Duration, Instant};
use err_derive::Error;
use rapier3d::geometry::InteractionGroups;
use simba::scalar::SubsetOf;
use vulkano::{descriptor_set, memory, sync};
use vulkano::buffer::{BufferUsage, ImmutableBuffer, TypedBufferAccess};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::{DescriptorSet, PersistentDescriptorSet};
use vulkano::pipeline::{GraphicsPipeline, PipelineBindPoint};

mod spawner;
mod remover;
mod axis;
mod ball_socket;
mod weld;
mod tool;
mod prop_manager;
mod rope;

use crate::application::{Application, Entity};
use crate::component::parent::Parent;
use crate::debug;
use crate::math::{AMat4, Color, Isometry3, Point3, Ray, Rot3, Similarity3, Vec3, cast_ray_on_plane};
use crate::renderer::pipelines::PipelineError;
use crate::renderer::pipelines::toolgun_text::{ToolGunTextPipeline, Vertex};
use crate::renderer::Renderer;
use crate::utils::FenceCheck;
use super::{Component, ComponentBase, ComponentError, ComponentInner, ComponentRef, RenderType};
use prop_manager::{PropCollection, PropManagerError};
use tool::{get_all_tools, Tool};
use crate::component::hand::HandComponent;

const MENU_SPACING: f32 = 0.1;
const MENU_DISTANCE: f32 = 0.1;

#[derive(Copy, Clone)]
pub struct ToolGunAnim {
	start: Instant,
	origin: Point3,
	target: Point3,
	scale: f32,
}

pub struct ToolGunState {
	scroll: f32,
	tools: Vec<Box<dyn Tool>>,
	tool_id: usize,
	menu_pos: Option<Isometry3>,
	render_tool: bool,
}

#[derive(ComponentBase)]
pub struct ToolGun {
	#[inner] inner: ComponentInner,
	state: RefCell<ToolGunState>,
	anim: Cell<Option<ToolGunAnim>>,
	prop_collection: PropCollection,
	parent: ComponentRef<Parent>,
	grab_pos: Isometry3,
	pipeline: Arc<GraphicsPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	set: Arc<dyn DescriptorSet + Send + Sync>,
	fence: FenceCheck,
}

impl ToolGun {
	pub fn new(grab_pos: Isometry3, renderer: &mut Renderer) -> Result<Self, ToolGunError> {
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
		
		let prop_manager = PropCollection::new(renderer)?;
		
		let state = ToolGunState {
			scroll: 0.0,
			tools: get_all_tools(),
			tool_id: 0,
			menu_pos: None,
			render_tool: false,
		};
		
		Ok(ToolGun {
			inner: ComponentInner::from_render_type(RenderType::Transparent),
			parent: ComponentRef::null(),
			state: RefCell::new(state),
			anim: Cell::new(None),
			prop_collection: prop_manager,
			grab_pos,
			pipeline,
			vertices,
			set,
			fence,
		})
	}
	
	pub fn ray(&self, application: &Application) -> Ray {
		let position = self.entity(application).state().position;
		
		Ray {
			origin: position.transform_point(&point!(0.002683, 0.038828, 0.150084)),
			dir: position.transform_vector(&vector!(0.0, 0.0, 1.0)),
		}
	}
	
	pub fn fire(&self, application: &Application) {
		let ray = self.ray(application);
		
		let result = {
			let physics = &*application.physics.borrow();
			physics.query_pipeline.cast_ray(&physics.collider_set, &ray, 9999.0, false, InteractionGroups::all(), None)
		};
		
		if let Some((_, toi)) = result {
			let hit = ray.point_at(toi);
			
			self.anim.set(Some(ToolGunAnim {
				start: Instant::now(),
				origin: ray.origin,
				target: hit,
				scale: (2.0 / toi).clamp(0.1, 5.0),
			}));
		}
	}
}

impl Component for ToolGun {
	fn start(&self, entity: &Entity, _application: &Application) -> Result<(), ComponentError> {
		entity.set_tag("GrabSticky", true);
		entity.set_tag("GrabPos", self.grab_pos);
		
		Ok(())
	}
	
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> {
		let ray = self.ray(application);
		let state = &mut *self.state.borrow_mut();
		
		let result = {
			let physics = &*application.physics.borrow();
			physics.query_pipeline.cast_ray(&physics.collider_set, &ray, 9999.0, false, InteractionGroups::all(), None)
		};
		
		if let Some((_, intersection)) = result {
			let hit = ray.point_at(intersection);
			
			debug::draw_point(hit, 32.0, Color::cyan());
		}
		
		state.render_tool = false;
		if let Some(hand_comp) = entity.tag("Grabbed")
		                               .and_then(|c: ComponentRef<HandComponent>| c.get(application)) {
			let hand = hand_comp.hand;
			
			if application.input.drop_btn(hand).down {
				entity.unset_tag("Grabbed");
				state.menu_pos = None;
			} else if let Some(menu_pos) = state.menu_pos {
				let select_id = cast_ray_on_plane(menu_pos, ray).map(|menu_hit|
					state.tool_id as isize - (menu_hit.y / MENU_SPACING).round() as isize
				);
				
				for (tool_id, tool) in state.tools.iter().enumerate() {
					let text_box_pos = menu_pos.transform_point(&point!(0.0, (state.tool_id as f32 - tool_id as f32) * MENU_SPACING, 0.0));
					
					let color = if Some(tool_id as isize) == select_id {
						Color::yellow()
					} else {
						Color::white()
					};
					
					debug::draw_text(tool.name(), text_box_pos, debug::DebugOffset::center(0.0, 0.0), 128.0, color);
				}
				
				if let Some(select_id) = select_id {
					if application.input.fire_btn(hand).down {
						state.menu_pos = None;
						if select_id >= 0 && select_id < state.tools.len() as isize  {
							state.tool_id = select_id as usize;
						}
					}
				}
				
				if application.input.use_btn(hand).down {
					state.menu_pos = None;
				}
			} else {
				state.tools[state.tool_id].tick(self, hand, ray, application)?;
				state.render_tool = true;
				
				if application.input.use_btn(hand).down {
					state.menu_pos = Some(Isometry3::face_towards(&ray.point_at(MENU_DISTANCE), &ray.origin, &Vec3::y_axis()));
				}
			}
		}
		
		if let Some(anim) = self.anim.get() {
			let elapsed = anim.start.elapsed().as_secs_f32();
			
			debug::draw_line(anim.origin, anim.target, 10.0 - elapsed * 50.0, Color::cyan().opactiy(elapsed * 5.0));
			debug::draw_point(anim.target, anim.scale * elapsed * 1000.0, Color::white().opactiy(1.0 - elapsed * 5.0));
			
			if elapsed > 2.0 {
				self.anim.set(None);
			}
		}
		
		state.scroll += delta_time.as_secs_f32();
		
		Ok(())
	}
	
	fn render(&self, entity: &Entity, renderer: &Renderer, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		if !self.fence.check() { return Ok(()); }
		let state = &mut *self.state.borrow_mut();
		
		let tool = state.tools.get_mut(state.tool_id);
		let text = tool.as_ref().map_or("None", |t| t.name());
		let text_entry = renderer.debug_text_cache().get(text)?;
		let text_pos = entity.state().position * Similarity3::from_parts(vector!(0.000671, 0.059217, -0.027263).into(),
		                                                                 Rot3::from_euler_angles(0.781855066, 0.0, 0.0),
		                                                                 0.02135);
		let text_ratio = text_entry.size.0 as f32 / text_entry.size.1 as f32;
		let model_matrix: AMat4 = text_pos.to_superset();
		
		let uv_transform = vector!(
			2.0 / text_ratio,
			2.0,
			(state.scroll * 2.0) % (text_ratio + 2.0) / (text_ratio + 2.0) * (2.0 / text_ratio + 1.0) - (2.0 / text_ratio),
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
		
		if state.render_tool {
			if let Some(tool) = tool {
				tool.render(self, builder)?;
			}
		}
		
		Ok(())
	}
	
	fn end(&self, _entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		if let Some(parent) = self.parent.get(application) {
			if let Some(controller) = parent.target.get(application) {
				controller.state_mut().hidden = false;
			}
		}
		
		Ok(())
	}
}


#[derive(Debug, Error)]
pub enum ToolGunError {
	#[error(display = "Pipeline doesn't have specified layout")] NoLayout,
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] PropManagerError(#[error(source)] PropManagerError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
	#[error(display = "{}", _0)] DescriptorSetError(#[error(source)] descriptor_set::DescriptorSetError),
}
