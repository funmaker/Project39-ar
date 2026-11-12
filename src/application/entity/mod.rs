use std::any::Any;
use std::cell::{Ref, RefCell, RefMut, Cell};
use std::collections::{HashMap, BTreeMap};
use std::fmt::{Display, Formatter};
use std::time::Duration;
use anyhow::Result;
use egui::Ui;
use rapier3d::dynamics::{RigidBody, RigidBodyHandle, RigidBodyType};
use rapier3d::pipeline::PhysicsWorld;

mod builder;
mod entity_ref;

use crate::debug;
use crate::component::ComponentRef;
use crate::math::{Color, Isometry3, Point3, Vec3};
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::utils::{IntoBoxed, get_user_data, MutMark, InspectObject, GetSet, ExUi, SimpleInspect, ref_cell_iter, SelectionLevel};
use super::{Application, Component, Hand};
pub use builder::EntityBuilder;
pub use entity_ref::EntityRef;


/// Transform and visibility state of an entity.
///
/// Values are wrapped in `MutMark` so that changes made by components are detected and synced
/// to the rigid body before the next physics step.
pub struct EntityState {
	pub position: MutMark<Isometry3>,
	pub velocity: MutMark<Vec3>,
	pub angular_velocity: MutMark<Vec3>,
	pub hidden: bool,
}

/// A scene object: a rigid body with a transform, a parent/children hierarchy, string-keyed tags
/// and a set of components.
///
/// Create entities using `Entity::builder` and add them with `Application::add_entity`.
pub struct Entity {
	pub id: u64,
	pub name: String,
	pub rigid_body: RigidBodyHandle,
	/// Arbitrary typed tags (eg. `"World"`, `"NoGrab"`, `"Hand"`) used to mark entities.
	pub tags: RefCell<HashMap<String, Box<dyn Any>>>,
	parent: EntityRef,
	/// Offset from the parent, if this entity follows its parent's movement.
	parent_offset: Cell<Option<Isometry3>>,
	children: RefCell<Vec<EntityRef>>,
	state: RefCell<EntityState>,
	initialized: Cell<bool>,
	removed: Cell<bool>,
	persist: Cell<bool>,
	/// Set when the entity was frozen by `freeze` (dynamic body turned fixed).
	frozen: Cell<bool>,
	components: BTreeMap<u64, Box<dyn Component>>,
	/// Components added but not yet inserted into `components`.
	new_components: RefCell<Vec<Box<dyn Component>>>,
	/// Rigid body inserted into the physics world on `initialize`.
	rigid_body_template: RigidBody,
}

impl Entity {
	pub fn builder(name: impl Into<String>) -> EntityBuilder {
		EntityBuilder::new(name)
	}
	
	/// Inserts the entity's rigid body into the physics world and attaches it to its parent.
	/// Called by the application when the entity is added to the scene.
	pub fn initialize(&mut self, application: &Application) {
		{
			let mut physics = application.physics.borrow_mut();
			let state = self.state.get_mut();
			let mut rb = self.rigid_body_template.clone();
			
			rb.user_data = get_user_data(self.id, 0);
			rb.set_position((*state.position).into(), true);
			rb.set_linvel((*state.velocity).into(), true);
			rb.set_angvel((*state.angular_velocity).into(), true);
			
			self.rigid_body = physics.insert_body(rb);
		}
		
		self.initialized.set(true);
		
		self.set_parent(self.parent.clone(), self.parent_offset.get().is_some(), application);
	}
	
	/// Moves pending components into the entity. Returns `true` if any component was added.
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
	
	/// Calls `start` on all components that have not been started yet.
	pub fn setup_new_components(&self, application: &Application) -> Result<()> {
		for component in self.components.values() {
			if component.inner().is_new() {
				component.inner().mark_started();
				component.start(self, application)?;
			}
		}
		
		Ok(())
	}
	
	/// Syncs state changes made by components (or the parent's movement) into the rigid body
	/// before the physics step.
	pub fn before_physics(&self, application: &Application, physics: &mut PhysicsWorld) {
		let mut state = self.state_mut();
		let rigid_body = self.rigid_body_mut(physics);
		
		if state.position.was_mut() {
			if self.parent_offset.get().is_some() {
				if let Some(parent) = self.parent.get(application) {
					self.parent_offset.set(Some(parent.state().position.inverse() * *state.position));
				}
			}
			
			rigid_body.set_position((*state.position).into(), true);
		} else if let Some(parent_offset) = self.parent_offset.get() {
			if let Some(parent) = self.parent.get(application) {
				if parent.state().position.was_mut() {
					*state.position = *parent.state().position * parent_offset;
					
					rigid_body.set_position((*state.position).into(), true);
				}
			}
		}
		
		if state.velocity.was_mut() {
			rigid_body.set_linvel((*state.velocity).into(), true);
		}
		
		if state.angular_velocity.was_mut() {
			rigid_body.set_angvel((*state.angular_velocity).into(), true);
		}
	}
	
	/// Keeps following entities attached to their parent and copies the simulated rigid body state
	/// back into the entity state.
	pub fn after_physics(&self, application: &Application, physics: &mut PhysicsWorld) {
		if let Some(parent) = self.parent.get(application) {
			if let Some(parent_offset) = self.parent_offset.get() {
				let parent_sleeping = parent.rigid_body(physics).is_sleeping();
				let rigid_body = self.rigid_body_mut(physics);
				
				if !parent_sleeping {
					self.rigid_body_mut(physics).set_position((*parent.state().position * parent_offset).into(), true);
				} else if !rigid_body.is_sleeping() {
					self.rigid_body_mut(physics).sleep();
				}
			}
		}
		
		let mut state = self.state_mut();
		let rigid_body = self.rigid_body(physics);
		
		*state.position = (*rigid_body.position()).into();
		state.position.clear_mut();
		
		*state.velocity = rigid_body.linvel().into();
		state.velocity.clear_mut();
		
		*state.angular_velocity = rigid_body.angvel().into();
		state.angular_velocity.clear_mut();
	}
	
	/// Updates the position relative to the parent, ticks all components and draws the entity debug gizmo.
	pub fn tick(&self, delta_time: Duration, application: &Application) -> Result<()> {
		if let Some(parent) = self.parent.get(application) {
			if let Some(parent_offset) = self.parent_offset.get() {
				if parent.state().position.was_mut() {
					*self.state_mut().position = *parent.state().position * parent_offset;
				}
			}
		}
		
		for component in self.components.values() {
			component.tick(&self, &application, delta_time)?;
		}
		
		if debug::get_flag_or_default("DebugEntityDraw") {
			let state = self.state();
			let pos = *state.position;
			let size = debug::gizmo_scale(pos);
			let selection_level = application.gui_selections_test(self, false);
			let hide_unselected = debug::get_flag_or_default::<bool>("DebugHideUnselected");
			
			if let Some((stroke_opacity, _)) = debug::gizmo_opacity(selection_level.select_within, selection_level.hover_within, hide_unselected) {
				debug::draw_point(pos, 16.0, Color::MAGENTA.opactiy(stroke_opacity));
				debug::draw_line(pos, pos * Point3::from(Vec3::x() * size * 0.3), 4.0, Color::RED.opactiy(stroke_opacity));
				debug::draw_line(pos, pos * Point3::from(Vec3::y() * size * 0.3), 4.0, Color::GREEN.opactiy(stroke_opacity));
				debug::draw_line(pos, pos * Point3::from(Vec3::z() * size * 0.3), 4.0, Color::BLUE.opactiy(stroke_opacity));
				debug::draw_text(&self.name, pos, debug::DebugOffset::bottom_right(32.0, 32.0), 128.0, Color::MAGENTA.opactiy(stroke_opacity));
			}
		}
		
		Ok(())
	}
	
	/// Calls `before_render` on all components. Returns `true` if any component renders transparent geometry.
	pub fn before_render(&mut self, context: &mut RenderContext, renderer: &mut Renderer) -> Result<bool> {
		let mut is_transparent = false;
		
		for component in self.components.values() {
			component.before_render(&self, context, renderer)?;
			
			is_transparent = is_transparent || component.inner().render_type().contains(RenderType::Transparent);
		}
		
		Ok(is_transparent)
	}
	
	/// Renders all components that match the current render type, unless the entity is hidden
	/// (or tagged `"CloseHide"` and too close to the camera).
	pub fn render(&mut self, context: &mut RenderContext, renderer: &mut Renderer) -> Result<()> {
		let close_hide: bool = self.tag("CloseHide").unwrap_or_default();
		let state = self.state.get_mut();
		
		if state.hidden || (close_hide && (state.position.translation.vector - context.camera_pos.translation.vector).magnitude_squared() < 0.125) {
			return Ok(());
		}
		
		for component in self.components.values() {
			if component.inner().render_type().contains(context.render_type) {
				component.render(&self, context, renderer)?;
			}
		}
		
		Ok(())
	}
	
	/// Calls `end` on components marked for removal. Returns `true` if any component was ended.
	pub fn end_components(&self, application: &Application) -> Result<bool> {
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
	
	/// Drops components that have already been ended.
	pub fn cleanup_ended_components(&mut self) {
		self.components.retain(|_, component| !component.inner().is_dead());
	}
	
	/// Removes the entity's rigid body (and its attached colliders and joints) from the physics world.
	pub fn cleanup_physics(&mut self, physics: &mut PhysicsWorld) {
		physics.remove_body(self.rigid_body);
	}
	
	/// Draws the entity inspector: state, rigid body, components and tags.
	pub fn on_gui(&self, ui: &mut Ui, application: &Application) {
		use crate::utils::ExUi;
		use egui::*;
		
		let selection_level = self.selection_level(&application);
		ui.highlight_indent(selection_level);
		
		if selection_level.focus {
			ui.scroll_to_cursor(Some(Align::Min));
		}
		
		CollapsingHeader::new(RichText::new(&self.name).strong())
			.id_salt("Entity")
			.default_open(true)
			.open(selection_level.select_within.then_some(true))
			.show(ui, |ui| {
				ui.reset_style();
				
				Grid::new("Entity Parent")
					.num_columns(2)
					.min_col_width(100.0)
					.show(ui, |ui| {
						ui.inspect_row("ID", &self.as_ref(), application);
						ui.inspect_row("Follow Parent", GetSet(|| (
							self.parent_offset.get().is_some(),
							|follow| self.set_parent(self.parent.clone(), follow, application),
						)), ());
						ui.inspect_row("Parent Offset", &self.parent_offset, ());
					});
				
				ui.inspect(&mut *self.state_mut(), ());
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
	
	/// Returns the component with the given id if it exists and is of type `C`.
	pub fn component<C: Sized + 'static>(&self, id: u64) -> Option<&C> {
		self.components
		    .get(&id)
		    .and_then(|c| c.as_any().downcast_ref::<C>())
	}
	
	/// Returns `true` if a component with the given id was added but is not yet set up.
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
	
	/// Returns the first component of type `C`.
	pub fn find_component_by_type<C: Sized + 'static>(&self) -> Option<&C> {
		self.components
			.values()
			.find(|c| c.as_any().is::<C>())
			.and_then(|c| c.as_any().downcast_ref::<C>())
	}
	
	/// Iterates over all components of type `C`.
	pub fn iter_component_by_type<C: Sized + 'static>(&self) -> impl Iterator<Item = &C> {
		self.components
		    .values()
		    .filter_map(|c| c.as_any().downcast_ref::<C>())
	}
	
	/// Queues a component to be added to the entity. It will be started on the next `setup_loop`.
	pub fn add_component<C: IntoBoxed<dyn Component>>(&self, component: C) -> ComponentRef<C> {
		let component = component.into();
		let id = component.id();
		
		self.new_components.borrow_mut().push(component);
		
		ComponentRef::new(self.id, id)
	}
	
	pub fn is_being_removed(&self) -> bool {
		self.removed.get()
	}
	
	/// Marks the entity and all its components for removal. The removal happens in the cleanup loop.
	///
	/// Returns `false` if the entity was already being removed.
	pub fn remove(&self) -> bool {
		for component in self.components.values() {
			component.remove();
		}
		
		!self.removed.replace(true)
	}
	
	pub fn parent(&self) -> EntityRef {
		self.parent.clone()
	}
	
	/// Returns the topmost ancestor of this entity (or the entity itself).
	pub fn root<'a, 'b: 'a>(&'b self, application: &'a Application) -> &'a Entity {
		let mut parent = self;
		
		while let Some(grand_parent) = parent.parent().get(application) {
			parent = grand_parent;
		}
		
		parent
	}
	
	/// Sets the parent and the parent offset directly. `None` offset means the entity won't follow the parent.
	pub fn set_parent_and_offset(&self, parent: EntityRef, parent_offset: Option<Isometry3>, application: &Application) {
		self.unset_parent(application);
		
		self.parent.set(parent.clone());
		self.parent_offset.set(parent_offset);
		
		if let Some(new_parent) = parent.get(application) {
			new_parent.children.borrow_mut().push(self.as_ref());
		}
	}
	
	/// Sets the parent of this entity. If `follow` is set, the entity keeps its current offset from
	/// the parent and moves with it.
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
	
	/// Detaches the entity from its parent.
	pub fn unset_parent(&self, application: &Application) {
		if let Some(old_parent) = self.parent.get(application) {
			old_parent.children.borrow_mut().retain(|entity| entity != self);
		}
		
		self.parent.set(EntityRef::null());
	}
	
	pub fn children(&self) -> Ref<'_, [EntityRef]> {
		Ref::map(self.children.borrow(), Vec::as_slice)
	}
	
	/// Recursively iterates over all descendants of this entity.
	pub fn descendants<'a>(&'a self, application: &'a Application) -> impl Iterator<Item=&'a Entity> + 'a {
		ref_cell_iter(&self.children, Vec::as_slice)
			.flat_map(move |child| child.get(application))
			.flat_map(move |child|
				Some(child).into_iter()
				           .chain(Box::new(child.descendants(application)) as Box<dyn Iterator<Item=_>>)
			)
	}
	
	/// Returns `true` if the entity survives the removal of its parent.
	pub fn persists(&self) -> bool {
		self.persist.get()
	}
	
	pub fn set_persist(&self, persist: bool) { self.persist.set(persist) }
	
	pub fn state(&self) -> Ref<'_, EntityState> {
		self.state.borrow()
	}
	
	pub fn state_mut(&self) -> RefMut<'_, EntityState> {
		self.state.borrow_mut()
	}
	
	/// Like `state`, but returns `None` if the state is currently mutably borrowed.
	pub fn try_state(&self) -> Option<Ref<'_, EntityState>> {
		self.state.try_borrow().ok()
	}
	
	/// Like `state_mut`, but returns `None` if the state is currently borrowed.
	pub fn try_state_mut(&self) -> Option<RefMut<'_, EntityState>> {
		self.state.try_borrow_mut().ok()
	}
	
	pub fn rigid_body<'p>(&self, physics: &'p PhysicsWorld) -> &'p RigidBody {
		physics.bodies.get(self.rigid_body).unwrap()
	}
	
	pub fn rigid_body_mut<'p>(&self, physics: &'p mut PhysicsWorld) -> &'p mut RigidBody {
		physics.bodies.get_mut(self.rigid_body).unwrap()
	}
	
	/// Returns a clone of the tag value if it exists and has type `T`.
	pub fn tag<T: Clone + 'static>(&self, key: &str) -> Option<T> {
		self.tags.borrow().get(key).and_then(|b| b.downcast_ref().cloned())
	}
	
	/// Sets a tag, returning the previous value.
	pub fn set_tag<T: Clone + 'static>(&self, key: impl Into<String>, val: T) -> Option<Box<dyn Any>> {
		self.tags.borrow_mut().insert(key.into(), Box::new(val))
	}
	
	pub fn unset_tag(&self, key: &str) -> Option<Box<dyn Any>> {
		self.tags.borrow_mut().remove(key)
	}
	
	pub fn has_tag(&self, key: &str) -> bool {
		self.tags.borrow_mut().contains_key(key)
	}
	
	/// Turns a dynamic rigid body into a fixed one. Returns `true` if the body was frozen.
	pub fn freeze(&self, physics: &mut PhysicsWorld) -> bool {
		let rb = self.rigid_body_mut(physics);
		
		if rb.body_type() == RigidBodyType::Dynamic {
			rb.set_body_type(RigidBodyType::Fixed, true);
			self.frozen.set(true);
			true
		} else { false }
	}
	
	/// Restores a body frozen with `freeze` back to dynamic. Returns `true` if it was frozen.
	pub fn unfreeze(&self, physics: &mut PhysicsWorld) -> bool {
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

impl SimpleInspect for EntityState {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		use crate::utils::ExUi;
		use egui::*;
		
		Grid::new("Entity State")
			.num_columns(2)
			.min_col_width(100.0)
			.show(ui, |ui| {
				ui.inspect_row("Hidden", &mut self.hidden, ());
				ui.inspect_row("Position", &mut self.position, ());
				ui.inspect_row("Velocity", &mut self.velocity, ());
				ui.inspect_row("Angular Velocity", &mut self.angular_velocity, ());
			});
	}
}
