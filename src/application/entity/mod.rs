use std::cell::{Ref, RefCell, RefMut, Cell};
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
use std::any::Any;
use std::fmt::{Display, Formatter};
use rapier3d::prelude::{RigidBody, RigidBodyHandle, RigidBodyType};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};

mod builder;
mod entity_ref;

use crate::debug;
use crate::math::{Color, Isometry3, Point3, Vec3};
use crate::component::{ComponentRef, ComponentError, RenderType};
use crate::utils::{IntoBoxed, get_userdata};
use crate::renderer::Renderer;
use super::{Application, Component, Physics};
pub use builder::EntityBuilder;
pub use entity_ref::EntityRef;

pub struct EntityState {
	pub position: Isometry3,
	pub velocity: Vec3,
	pub angular_velocity: Vec3,
	pub hidden: bool,
}

pub struct Entity {
	pub id: u64,
	pub name: String,
	pub rigid_body: RigidBodyHandle,
	pub tags: RefCell<HashMap<String, Box<dyn Any>>>,
	rigid_body_template: RigidBody,
	state: RefCell<EntityState>,
	removed: Cell<bool>,
	frozen: Cell<bool>,
	components: BTreeMap<u64, Box<dyn Component>>,
	new_components: RefCell<Vec<Box<dyn Component>>>,
	mass: Cell<f32>,
}

impl Entity {
	pub fn builder(name: impl Into<String>) -> EntityBuilder {
		EntityBuilder::new(name)
	}
	
	pub fn remove(&self) -> bool {
		for component in self.components.values() {
			component.remove();
		}
		
		!self.removed.replace(true)
	}
	
	pub fn setup_physics(&mut self, physics: &mut Physics) {
		let mut rb = self.rigid_body_template.clone();
		rb.user_data = get_userdata(self.id, 0);
		self.rigid_body = physics.rigid_body_set.insert(rb);
	}
	
	// Safety note. These argument combination is not safe.
	// Immutable reference to Application can be used to create immutable reference to Self while &mut Self exists.
	// Do not use both at the same time.
	pub fn setup_components(&mut self, application: &Application) -> Result<bool, ComponentError> {
		let mut did_work = false;
		
		while let Some(mut component) = self.new_components.get_mut().pop() {
			did_work = true;
			
			let component_id = component.id();
			component.inner_mut().set_entity_id(self.id);
			
			let old = self.components.insert(component_id, component);
			assert!(old.is_none(), "Component id {} already taken in entity {}!", component_id, self.id);
			
			self.components.get(&component_id).unwrap().start(&self, application)?;
		}
		
		Ok(did_work)
	}
	
	pub fn before_physics(&self, physics: &mut Physics) {
		let state = self.state();
		let rigid_body = self.rigid_body_mut(physics);
		
		// TODO: Optimize wakey wakey
		rigid_body.set_position(state.position, true);
		rigid_body.set_linvel(state.velocity, true);
		rigid_body.set_angvel(state.angular_velocity, true);
	}
	
	pub fn after_physics(&self, physics: &mut Physics) {
		let mut state = self.state_mut();
		let rigid_body = self.rigid_body(physics);
		
		state.position = *rigid_body.position();
		state.velocity = *rigid_body.linvel();
		state.angular_velocity = *rigid_body.angvel();
	}
	
	pub fn tick(&self, delta_time: Duration, application: &Application) -> Result<(), ComponentError> {
		for component in self.components.values() {
			component.tick(&self, &application, delta_time)?;
		}
		
		self.mass.set(application.physics.borrow_mut().rigid_body_set.get(self.rigid_body).unwrap().mass());
		
		Ok(())
	}
	
	pub fn pre_render(&mut self, renderer: &Renderer, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<bool, ComponentError> {
		let mut is_transparent = false;
		
		for component in self.components.values() {
			component.pre_render(&self, renderer, builder)?;
			
			is_transparent = is_transparent || component.inner().is_transparent();
		}
		
		Ok(is_transparent)
	}
	
	pub fn render(&mut self, renderer: &Renderer, render_type: RenderType, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		let state = self.state.get_mut();
		
		if state.hidden {
			return Ok(());
		}
		
		if debug::get_flag_or_default("DebugEntityDraw") && render_type == RenderType::Opaque {
			let pos: Point3 = state.position.translation.vector.into();
			let ang = &state.position.rotation;
			
			debug::draw_point(&pos, 32.0, Color::magenta());
			debug::draw_line(&pos, &pos + ang * Vec3::x() * 0.3, 4.0, Color::red());
			debug::draw_line(&pos, &pos + ang * Vec3::y() * 0.3, 4.0, Color::green());
			debug::draw_line(&pos, &pos + ang * Vec3::z() * 0.3, 4.0, Color::blue());
			debug::draw_text(&self.name, &pos, debug::DebugOffset::bottom_right(32.0, 32.0), 128.0, Color::magenta());
			debug::draw_text(&format!("{}", self.mass.get()), &pos, debug::DebugOffset::bottom_right(32.0, 160.0), 128.0, Color::magenta());
		}
		
		for component in self.components.values() {
			if (render_type == RenderType::Opaque && component.inner().is_opaque())
			|| (render_type == RenderType::Transparent && component.inner().is_transparent()) {
				component.render(&self, renderer, builder)?;
			}
		}
		
		Ok(())
	}
	
	pub fn cleanup_components(&mut self, application: &Application) -> Result<bool, ComponentError> {
		let mut did_work = false;
		let mut clean = false;
		
		while !clean {
			clean = true;
			
			for component in self.components.values() {
				if component.inner().is_being_removed() {
					component.end(&self, application)?;
					clean = false;
					did_work = true;
				}
			}
			
			self.components.drain_filter(|_, component| component.inner().is_being_removed());
		}
		
		Ok(did_work)
	}
	
	pub fn cleanup_physics(&mut self, physics: &mut Physics) {
		physics.rigid_body_set.remove(self.rigid_body, &mut physics.island_manager, &mut physics.collider_set, &mut physics.impulse_joint_set, &mut physics.multibody_joint_set, true);
	}
	
	pub fn as_ref(&self) -> EntityRef {
		self.into()
	}
	
	pub fn is_being_removed(&self) -> bool {
		self.removed.get()
	}
	
	pub fn component<C: Sized + 'static>(&self, id: u64) -> Option<&C> {
		self.components
		    .get(&id)
		    .and_then(|c| c.as_any().downcast_ref::<C>())
	}
	
	pub fn find_component_by_type<C: Sized + 'static>(&self) -> Option<&C> {
		self.components
			.values()
			.find(|c| c.as_any().is::<C>())
			.and_then(|c| c.as_any().downcast_ref::<C>())
	}
	
	pub fn add_component<C: IntoBoxed<dyn Component>>(&self, component: C) -> ComponentRef<C> {
		let component = component.into();
		let id = component.id();
		
		self.new_components.borrow_mut().push(component);
		
		ComponentRef::new(self.id, id)
	}
	
	pub fn state(&self) -> Ref<EntityState> {
		self.state.borrow()
	}
	
	pub fn state_mut(&self) -> RefMut<EntityState> {
		self.state.borrow_mut()
	}
	
	pub fn try_state(&self) -> Option<Ref<EntityState>> {
		self.state.try_borrow().ok()
	}
	
	pub fn try_state_mut(&self) -> Option<RefMut<EntityState>> {
		self.state.try_borrow_mut().ok()
	}
	
	pub fn rigid_body<'p>(&self, physics: &'p Physics) -> &'p RigidBody {
		physics.rigid_body_set.get(self.rigid_body).unwrap()
	}
	
	pub fn rigid_body_mut<'p>(&self, physics: &'p mut Physics) -> &'p mut RigidBody {
		physics.rigid_body_set.get_mut(self.rigid_body).unwrap()
	}
	
	pub fn tag<T: Clone + 'static>(&self, key: &str) -> Option<T> {
		self.tags.borrow().get(key).and_then(|b| b.downcast_ref().cloned())
	}
	
	pub fn set_tag<T: Clone + 'static>(&self, key: impl Into<String>, val: T) -> Option<Box<dyn Any>> {
		self.tags.borrow_mut().insert(key.into(), Box::new(val))
	}
	
	pub fn unset_tag(&self, key: &str) -> Option<Box<dyn Any>> {
		self.tags.borrow_mut().remove(key)
	}
	
	pub fn has_tag(&self, key: &str) -> bool {
		self.tags.borrow_mut().contains_key(key)
	}
	
	pub fn freeze(&self, physics: &mut Physics) -> bool {
		let rb = self.rigid_body_mut(physics);
		
		if rb.body_type() == RigidBodyType::Dynamic {
			rb.set_body_type(RigidBodyType::Fixed);
			self.frozen.set(true);
			true
		} else { false }
	}
	
	pub fn unfreeze(&self, physics: &mut Physics) -> bool {
		if self.frozen.replace(false) {
			self.rigid_body_mut(physics)
			    .set_body_type(RigidBodyType::Dynamic);
			true
		} else { false }
	}
}

impl PartialEq<Self> for &Entity {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
	}
}

impl Eq for &Entity {}

impl Display for Entity {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}({})", self.name, self.id)
	}
}
