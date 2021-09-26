use std::cell::{Ref, RefCell, RefMut, Cell};
use std::collections::BTreeMap;
use std::time::Duration;
use rapier3d::prelude::{RigidBody, RigidBodyHandle};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};

mod builder;

use crate::debug;
use crate::math::{Color, Isometry3, Point3, Vec3};
use crate::component::{ComponentRef, ComponentError};
use crate::utils::IntoBoxed;
use crate::renderer::Renderer;
use super::{Application, Component, Physics};
pub use builder::EntityBuilder;

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
	rigid_body_template: RigidBody,
	state: RefCell<EntityState>,
	removed: Cell<bool>,
	components: BTreeMap<u64, Box<dyn Component>>,
	new_components: RefCell<Vec<Box<dyn Component>>>,
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
	
	pub fn is_being_removed(&self) -> bool {
		self.removed.get()
	}
	
	pub fn component<C: Sized + 'static>(&self, id: u64) -> Option<&C> {
		self.components
		    .get(&id)
		    .and_then(|c| c.as_any().downcast_ref::<C>())
	}
	
	pub fn setup_physics(&mut self, physics: &mut Physics) {
		self.rigid_body = physics.rigid_body_set.insert(self.rigid_body_template.clone());
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
		
		rigid_body.set_position(state.position, false);
		rigid_body.set_linvel(state.velocity, false);
		rigid_body.set_angvel(state.angular_velocity, false);
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
		
		Ok(())
	}
	
	pub fn pre_render(&mut self, renderer: &Renderer, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		for component in self.components.values() {
			component.pre_render(&self, renderer, builder)?;
		}
		
		Ok(())
	}
	
	pub fn render(&mut self, renderer: &Renderer, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		let state = self.state.get_mut();
		
		if state.hidden {
			return Ok(());
		}
		
		let pos: Point3 = state.position.translation.vector.into();
		let ang = &state.position.rotation;
		
		debug::draw_point(&pos, 32.0, Color::magenta());
		debug::draw_line(&pos, &pos + ang * Vec3::x() * 0.3, 4.0, Color::red());
		debug::draw_line(&pos, &pos + ang * Vec3::y() * 0.3, 4.0, Color::green());
		debug::draw_line(&pos, &pos + ang * Vec3::z() * 0.3, 4.0, Color::blue());
		debug::draw_text(&self.name, &pos, debug::DebugOffset::bottom_right(32.0, 32.0), 128.0, Color::magenta());
		
		for component in self.components.values() {
			component.render(&self, renderer, builder)?;
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
		physics.rigid_body_set.remove(self.rigid_body, &mut physics.island_manager, &mut physics.collider_set, &mut physics.joint_set);
	}
	
	pub fn as_ref(&self) -> EntityRef {
		self.into()
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
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct EntityRef {
	inner: Cell<Option<u64>>,
}

impl EntityRef {
	pub fn new(eid: u64) -> Self {
		EntityRef {
			inner: Cell::new(Some(eid)),
		}
	}
	
	pub fn null() -> Self {
		EntityRef {
			inner: Cell::new(None),
		}
	}
	
	pub fn set(&self, other: Self) {
		self.inner.swap(&other.inner);
	}
	
	pub fn get<'a>(&self, application: &'a Application) -> Option<&'a Entity> {
		if let Some(eid) = self.inner.get() {
			if let Some(entity) = application.entity(eid) {
				Some(entity)
			} else {
				self.inner.set(None);
				None
			}
		} else {
			None
		}
	}
}

impl PartialEq<Self> for &Entity {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id
	}
}

impl Eq for &Entity {}

impl PartialEq<&EntityRef> for &Entity {
	fn eq(&self, other: &&EntityRef) -> bool {
		if let Some(id) = other.inner.get() {
			self.id == id
		} else {
			false
		}
	}
}

impl From<&Entity> for EntityRef {
	fn from(entity: &Entity) -> Self {
		EntityRef::new(entity.id)
	}
}

impl From<&EntityRef> for EntityRef {
	fn from(entity_ref: &EntityRef) -> Self {
		entity_ref.clone()
	}
}
