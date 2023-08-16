use std::any::Any;
use std::cell::{Ref, RefCell, RefMut, Cell};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::time::Duration;
use egui::Ui;
use rapier3d::prelude::{RigidBody, RigidBodyHandle, RigidBodyType};

mod builder;
mod entity_ref;

use crate::debug;
use crate::component::{ComponentRef, ComponentError};
use crate::math::{Color, Isometry3, Point3, Vec3};
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::utils::{IntoBoxed, get_user_data, MutMark, InspectObject, GetSet};
use super::{Application, Component, Physics, Hand};
pub use builder::EntityBuilder;
pub use entity_ref::EntityRef;


pub struct EntityState {
	pub position: MutMark<Isometry3>,
	pub velocity: MutMark<Vec3>,
	pub angular_velocity: MutMark<Vec3>,
	pub hidden: bool,
}

pub struct Entity {
	pub id: u64,
	pub name: String,
	pub rigid_body: RigidBodyHandle,
	pub tags: RefCell<HashMap<String, Box<dyn Any>>>,
	parent: EntityRef,
	parent_offset: Cell<Option<Isometry3>>,
	children: RefCell<Vec<EntityRef>>,
	state: RefCell<EntityState>,
	initialized: Cell<bool>,
	removed: Cell<bool>,
	persist: Cell<bool>,
	frozen: Cell<bool>,
	components: HashMap<u64, Box<dyn Component>>,
	new_components: RefCell<Vec<Box<dyn Component>>>,
	rigid_body_template: RigidBody,
}

impl Entity {
	pub fn builder(name: impl Into<String>) -> EntityBuilder {
		EntityBuilder::new(name)
	}
	
	pub fn initialize(&mut self, application: &Application) {
		{
			let mut physics = application.physics.borrow_mut();
			let state = self.state.get_mut();
			let mut rb = self.rigid_body_template.clone();
			
			rb.user_data = get_user_data(self.id, 0);
			rb.set_position(*state.position, true);
			rb.set_linvel(*state.velocity, true);
			rb.set_angvel(*state.angular_velocity, true);
			
			self.rigid_body = physics.rigid_body_set.insert(rb);
		}
		
		self.initialized.set(true);
		
		self.set_parent(self.parent.clone(), self.parent_offset.get().is_some(), application);
	}
	
	pub fn add_new_components(&mut self) -> bool {
		let mut did_work = false;
		
		for mut component in self.new_components.get_mut().drain(..) {
			did_work = true;
			
			let component_id = component.id();
			component.inner_mut().set_entity_id(self.id);
			
			let old = self.components.insert(component_id, component);
			assert!(old.is_none(), "Component id {} already taken in entity {}!", component_id, self.id);
		}
		
		did_work
	}
	
	pub fn setup_new_components(&self, application: &Application) -> Result<(), ComponentError> {
		for component in self.components.values() {
			if component.inner().is_new() {
				component.inner().mark_started();
				component.start(self, application)?;
			}
		}
		
		Ok(())
	}
	
	pub fn before_physics(&self, application: &Application, physics: &mut Physics) {
		let mut state = self.state_mut();
		let rigid_body = self.rigid_body_mut(physics);
		
		if state.position.mutated {
			if self.parent_offset.get().is_some() {
				if let Some(parent) = self.parent.get(application) {
					self.parent_offset.set(Some(parent.state().position.inverse() * *state.position));
				}
			}
			
			rigid_body.set_position(*state.position, true);
		} else if let Some(parent_offset) = self.parent_offset.get() {
			if let Some(parent) = self.parent.get(application) {
				if parent.state().position.mutated {
					*state.position = *parent.state().position * parent_offset;
					
					rigid_body.set_position(*state.position, true);
				}
			}
		}
		
		if state.velocity.mutated {
			rigid_body.set_linvel(*state.velocity, true);
		}
		
		if state.angular_velocity.mutated {
			rigid_body.set_angvel(*state.angular_velocity, true);
		}
	}
	
	pub fn after_physics(&self, application: &Application, physics: &mut Physics) {
		if let Some(parent) = self.parent.get(application) {
			if let Some(parent_offset) = self.parent_offset.get() {
				let parent_sleeping = parent.rigid_body(physics).is_sleeping();
				let rigid_body = self.rigid_body_mut(physics);
				
				if !parent_sleeping {
					self.rigid_body_mut(physics).set_position(*parent.state().position * parent_offset, true);
				} else if !rigid_body.is_sleeping() {
					self.rigid_body_mut(physics).sleep();
				}
			}
		}
		
		let mut state = self.state_mut();
		let rigid_body = self.rigid_body(physics);
		
		*state.position = *rigid_body.position();
		state.position.reset();
		
		*state.velocity = *rigid_body.linvel();
		state.velocity.reset();
		
		*state.angular_velocity = *rigid_body.angvel();
		state.angular_velocity.reset();
	}
	
	pub fn tick(&self, delta_time: Duration, application: &Application) -> Result<(), ComponentError> {
		if let Some(parent) = self.parent.get(application) {
			if let Some(parent_offset) = self.parent_offset.get() {
				if parent.state().position.mutated {
					*self.state_mut().position = *parent.state().position * parent_offset;
				}
			}
		}
		
		for component in self.components.values() {
			component.tick(&self, &application, delta_time)?;
		}
		
		Ok(())
	}
	
	pub fn before_render(&mut self, context: &mut RenderContext, renderer: &mut Renderer) -> Result<bool, ComponentError> {
		let mut is_transparent = false;
		
		for component in self.components.values() {
			component.before_render(&self, context, renderer)?;
			
			is_transparent = is_transparent || component.inner().render_type().contains(RenderType::Transparent);
		}
		
		Ok(is_transparent)
	}
	
	pub fn render(&mut self, context: &mut RenderContext, renderer: &mut Renderer) -> Result<(), ComponentError> {
		let close_hide: bool = self.tag("CloseHide").unwrap_or_default();
		let state = self.state.get_mut();
		
		if state.hidden || (close_hide && (state.position.translation.vector - context.camera_pos.translation.vector).magnitude_squared() < 0.125) {
			return Ok(());
		}
		
		if debug::get_flag_or_default("DebugEntityDraw") && context.render_type == RenderType::Opaque {
			let pos = *state.position;
			
			debug::draw_point(pos, 32.0, Color::MAGENTA);
			debug::draw_line(pos, pos * Point3::from(Vec3::x() * 0.3), 4.0, Color::RED);
			debug::draw_line(pos, pos * Point3::from(Vec3::y() * 0.3), 4.0, Color::GREEN);
			debug::draw_line(pos, pos * Point3::from(Vec3::z() * 0.3), 4.0, Color::BLUE);
			debug::draw_text(&self.name, pos, debug::DebugOffset::bottom_right(32.0, 32.0), 128.0, Color::MAGENTA);
		}
		
		for component in self.components.values() {
			if component.inner().render_type().contains(context.render_type) {
				component.render(&self, context, renderer)?;
			}
		}
		
		Ok(())
	}
	
	pub fn end_components(&self, application: &Application) -> Result<bool, ComponentError> {
		let mut did_work = false;
		
		for component in self.components.values() {
			if component.inner().is_being_removed() {
				component.inner().mark_dead();
				component.end(&self, application)?;
				did_work = true;
			}
		}
		
		Ok(did_work)
	}
	
	pub fn cleanup_ended_components(&mut self) {
		self.components.retain(|_, component| !component.inner().is_dead());
	}
	
	pub fn cleanup_physics(&mut self, physics: &mut Physics) {
		physics.rigid_body_set.remove(self.rigid_body, &mut physics.island_manager, &mut physics.collider_set, &mut physics.impulse_joint_set, &mut physics.multibody_joint_set, true);
	}
	
	pub fn on_gui(&self, ui: &mut Ui, application: &Application) {
		use crate::utils::ExUi;
		use egui::*;
		
		let selected = self.is_selected(&application);
		
		if selected {
			ui.highlight_indent();
			self.selection_scroll(ui, &application);
		}
		
		CollapsingHeader::new(RichText::new(&self.name).strong())
			.id_source("Entity")
			.default_open(true)
			.open(selected.then_some(true))
			.show(ui, |ui| {
				ui.reset_style();
				
				Grid::new("Entity State")
					.num_columns(2)
					.min_col_width(100.0)
					.show(ui, |ui| {
						let state = &mut *self.state_mut();
						
						ui.inspect_row("ID", &self.as_ref(), application);
						ui.inspect_row("Hidden", &mut state.hidden, ());
						ui.inspect_row("Position", &mut state.position, ());
						ui.inspect_row("Velocity", &mut state.velocity, ());
						ui.inspect_row("Angular Velocity", &mut state.angular_velocity, ());
						ui.inspect_row("Follow Parent", GetSet(|| (
							self.parent_offset.get().is_some(),
							|follow| self.set_parent(self.parent.clone(), follow, application),
						)), ());
						ui.inspect_row("Parent Offset", &self.parent_offset, ());
					});
			});
		
		ui.reset_style();
		
		ui.inspect_collapsing()
		  .title("Rigid Body")
		  .default_open(true)
		  .show(ui, self.rigid_body, application);
		
		for component in self.components.values() {
			ui.inspect_collapsing()
				.default_open(true)
				.show(ui, &component.as_cref_dyn(), application)
		}
		
		CollapsingHeader::new("Tags")
			.default_open(true)
			.show(ui, |ui| {
				Grid::new("Tags")
					.num_columns(2)
					.min_col_width(100.0)
					.show(ui, |ui| {
						for (name, value) in self.tags.borrow_mut().iter_mut() {
							ui.label(name);
							
							if let Some(value) = value.downcast_mut::<bool>() { ui.inspect(value, ()); }
							else if let Some(value) = value.downcast_mut::<Hand>() { ui.inspect(value, ()); }
							else { ui.label(RichText::new("Unknown Type").weak().italics()); }
							
							ui.allocate_space(ui.available_size());
							ui.end_row();
						}
					});
			});
	}
	
	pub fn as_ref(&self) -> EntityRef {
		self.into()
	}
	
	pub fn component<C: Sized + 'static>(&self, id: u64) -> Option<&C> {
		self.components
		    .get(&id)
		    .and_then(|c| c.as_any().downcast_ref::<C>())
	}
	
	pub fn pending_component(&self, id: u64) -> bool {
		self.new_components
			.borrow_mut()
			.iter()
			.any(|c| c.id() == id)
	}
	
	pub fn component_dyn(&self, id: u64) -> Option<&dyn Component> {
		self.components
		    .get(&id)
		    .map(|c| &**c)
	}
	
	pub fn find_component_by_type<C: Sized + 'static>(&self) -> Option<&C> {
		self.components
			.values()
			.find(|c| c.as_any().is::<C>())
			.and_then(|c| c.as_any().downcast_ref::<C>())
	}
	
	pub fn iter_component_by_type<C: Sized + 'static>(&self) -> impl Iterator<Item = &C> {
		self.components
		    .values()
		    .filter_map(|c| c.as_any().downcast_ref::<C>())
	}
	
	pub fn add_component<C: IntoBoxed<dyn Component>>(&self, component: C) -> ComponentRef<C> {
		let component = component.into();
		let id = component.id();
		
		self.new_components.borrow_mut().push(component);
		
		ComponentRef::new(self.id, id)
	}
	
	pub fn is_being_removed(&self) -> bool {
		self.removed.get()
	}
	
	pub fn remove(&self) -> bool {
		for component in self.components.values() {
			component.remove();
		}
		
		!self.removed.replace(true)
	}
	
	pub fn parent(&self) -> EntityRef {
		self.parent.clone()
	}
	
	pub fn root<'a, 'b: 'a>(&'b self, application: &'a Application) -> &'a Entity {
		let mut parent = self;
		
		while let Some(grand_parent) = parent.parent().get(application) {
			parent = grand_parent;
		}
		
		parent
	}
	
	pub fn set_parent_and_offset(&self, parent: EntityRef, parent_offset: Option<Isometry3>, application: &Application) {
		self.unset_parent(application);
		
		self.parent.set(parent.clone());
		self.parent_offset.set(parent_offset);
		
		if let Some(new_parent) = parent.get(application) {
			new_parent.children.borrow_mut().push(self.as_ref());
		}
	}
	
	pub fn set_parent(&self, parent: EntityRef, follow: bool, application: &Application) {
		self.unset_parent(application);
		
		self.parent.set(parent.clone());
		
		if self.initialized.get() {
			if let Some(new_parent) = parent.get(application) {
				new_parent.children.borrow_mut().push(self.as_ref());
				
				if follow {
					self.parent_offset.set(Some(new_parent.state().position.inverse() * *self.state().position));
				} else {
					self.parent_offset.set(None);
				}
			}
		} else {
			self.parent_offset.set(follow.then_some(Isometry3::identity()));
		}
	}
	
	pub fn unset_parent(&self, application: &Application) {
		if let Some(old_parent) = self.parent.get(application) {
			old_parent.children.borrow_mut().retain(|entity| entity != self);
		}
		
		self.parent.set(EntityRef::null());
	}
	
	pub fn children(&self) -> Ref<Vec<EntityRef>> {
		self.children.borrow()
	}
	
	pub fn persists(&self) -> bool {
		self.persist.get()
	}
	
	pub fn set_persist(&self, persist: bool) { self.persist.set(persist) }
	
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
			rb.set_body_type(RigidBodyType::Fixed, true);
			self.frozen.set(true);
			true
		} else { false }
	}
	
	pub fn unfreeze(&self, physics: &mut Physics) -> bool {
		if self.frozen.replace(false) {
			self.rigid_body_mut(physics)
			    .set_body_type(RigidBodyType::Dynamic, true);
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
