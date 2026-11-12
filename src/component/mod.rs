use std::any::Any;
use std::cell::Cell;
use std::fmt::{Formatter, Debug};
use std::marker::PhantomData;
use std::time::Duration;
pub use project39_ar_derive::ComponentBase;
use egui::{Grid, Ui};
use enumflags2::BitFlags;
use anyhow::Result;

pub mod comedy;
pub mod example;
pub mod glow;
pub mod hand;
pub mod katamari;
pub mod miku;
pub mod model;
pub mod pc_controlled;
pub mod physics;
pub mod pov;
pub mod seat;
pub mod srgb_test;
pub mod test;
pub mod thruster;
pub mod toolgun;
pub mod vr;

use crate::application::{Application, Entity, EntityRef};
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::utils::{next_uid, IntoBoxed, ExUi};


pub type ComponentError = Box<dyn std::error::Error>;

/// Bookkeeping part of a component. Implement it using `#[derive(ComponentBase)]` on a struct
/// with an `#[inner] inner: ComponentInner` field.
pub trait ComponentBase: Any {
	/// Returns the component's common state.
	fn inner(&self) -> &ComponentInner;
	fn inner_mut(&mut self) -> &mut ComponentInner;
	fn as_any(&self) -> &dyn Any;
	/// Name of the component type, displayed in the inspector.
	fn name(&self) -> &'static str;
	
	fn id(&self) -> u64 {
		self.inner().id
	}
	
	/// Marks the component for removal. `end` will be called during the cleanup loop.
	///
	/// Returns `false` if it was already being removed.
	fn remove(&self) -> bool {
		self.inner().mark_for_removal()
	}
	
	/// Returns the entity this component is attached to.
	///
	/// Panics if the component is not attached to a live entity yet.
	fn entity<'a>(&self, application: &'a Application) -> &'a Entity {
		let eid = self.inner().entity_id.expect("Attempted to get entity of unmounted component");
		application.entity(eid).expect("Attempted to get entity of unmounted component")
	}
	
	/// Returns a typed weak reference to this component.
	fn as_cref(&self) -> ComponentRef<Self> where Self: Sized {
		ComponentRef::new(self.inner().entity_id.expect("Attempted to get reference of unmounted component"), self.inner().id)
	}
	
	/// Returns an untyped weak reference to this component.
	fn as_cref_dyn(&self) -> ComponentRef<dyn Component> {
		ComponentRef::new(self.inner().entity_id.expect("Attempted to get reference of unmounted component"), self.inner().id)
	}
}

/// Behaviour attached to an entity. All hooks take `&self`, so mutable state has to use
/// `Cell`/`RefCell`. See `example.rs` for a minimal template.
#[allow(unused_variables)]
pub trait Component: ComponentBase {
	/// Called once, during the setup loop after the component was added.
	fn start(&self, entity: &Entity, application: &Application) -> Result<()> { Ok(()) }
	/// Called every frame after physics and GUI.
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<()> { Ok(()) }
	/// Called before rendering, for every render target. Prepare per-frame GPU data here.
	/// Only called if the component has a render type.
	fn before_render(&self, entity: &Entity, context: &mut RenderContext, renderer: &mut Renderer) -> Result<()> { Ok(()) }
	/// Records the component's draw commands. Called for each render type in `ComponentInner::render_type`.
	fn render(&self, entity: &Entity, context: &mut RenderContext, renderer: &mut Renderer) -> Result<()> { Ok(()) }
	/// Called once during the cleanup loop after the component was removed.
	fn end(&self, entity: &Entity, application: &Application) -> Result<()> { Ok(()) }
	/// Adds rows to the component's inspector grid.
	fn on_inspect(&self, entity: &Entity, ui: &mut Ui, application: &Application) {}
	/// Draws additional inspector UI below the inspector grid.
	fn on_inspect_extra(&self, entity: &Entity, ui: &mut Ui, application: &Application) {}
	/// Draws the full inspector of this component. Override `on_inspect`/`on_inspect_extra` instead,
	/// unless a custom layout is needed.
	fn on_gui(&self, entity: &Entity, ui: &mut Ui, application: &Application) {
		Grid::new(self.inner().id)
			.num_columns(2)
			.min_col_width(100.0)
			.show(ui, |ui| {
				ui.inspect_row("ID", &self.as_cref_dyn(), application);
				self.on_inspect(entity, ui, application);
			});
		
		self.on_inspect_extra(entity, ui, application);
	}
	
	/// Boxes the component as a `dyn Component`.
	fn boxed(self)
	         -> Box<dyn Component>
		where Self: Sized
	{ Box::new(self) }
}

impl IntoBoxed<dyn Component> for Box<dyn Component> {
	fn into(self) -> Box<dyn Component> {
		self
	}
}

impl<M: Component + 'static> IntoBoxed<dyn Component> for M {
	fn into(self) -> Box<dyn Component> {
		Box::new(self)
	}
}

/// Life cycle of a component: `New` → (`start`) → `Alive` → (`remove`) → `BeingRemoved` → (`end`) → `Dead`.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum ComponentLifeStage {
	New,
	Alive,
	BeingRemoved,
	Dead,
}

/// State common to all components: id, owning entity, render type and life stage.
#[derive(Debug)]
pub struct ComponentInner {
	id: u64,
	entity_id: Option<u64>,
	render_type: BitFlags<RenderType>,
	life_stage: Cell<ComponentLifeStage>,
}

impl ComponentInner {
	/// Creates the inner of a component that doesn't render anything.
	pub fn new_norender() -> Self {
		ComponentInner::from_render_type(BitFlags::empty())
	}
	
	/// Creates the inner of a component whose render hooks are called for the given render types.
	pub fn from_render_type(render_type: impl Into<BitFlags<RenderType>>) -> Self {
		ComponentInner {
			id: next_uid(),
			entity_id: None,
			render_type: render_type.into(),
			life_stage: Cell::new(ComponentLifeStage::New),
		}
	}
	
	/// Attaches the component to an entity. Panics if it is already attached.
	pub fn set_entity_id(&mut self, entity_id: u64) {
		assert!(self.entity_id.is_none(), "Component {} already mounted! Old: {} New: {}.", self.id, self.entity_id.unwrap(), entity_id);
		
		self.entity_id = Some(entity_id);
	}
	
	/// Render types the component's render hooks are called for.
	pub fn render_type(&self) -> BitFlags<RenderType> {
		self.render_type
	}
	
	pub fn life_stage(&self) -> ComponentLifeStage {
		self.life_stage.get()
	}
	
	pub fn is_new(&self) -> bool {
		self.life_stage.get() == ComponentLifeStage::New
	}
	
	pub fn is_being_removed(&self) -> bool {
		self.life_stage.get() == ComponentLifeStage::BeingRemoved
	}
	
	pub fn is_dead(&self) -> bool {
		self.life_stage.get() == ComponentLifeStage::Dead
	}
	
	/// Transitions `New` → `Alive`. Panics on any other stage.
	pub fn mark_started(&self) {
		match self.life_stage.replace(ComponentLifeStage::Alive) {
			ComponentLifeStage::New => {},
			ComponentLifeStage::Alive |
			ComponentLifeStage::BeingRemoved => panic!("Component {} in {:?} has already been started!", self.id, self.entity_id),
			ComponentLifeStage::Dead => panic!("Component {} is already dead!", self.id),
		}
	}
	
	/// Transitions to `Dead`. Panics if the component wasn't started or is already dead.
	pub fn mark_dead(&self) {
		match self.life_stage.replace(ComponentLifeStage::Dead) {
			ComponentLifeStage::New => panic!("Component {} has not been started yet!", self.id),
			ComponentLifeStage::Alive |
			ComponentLifeStage::BeingRemoved => {},
			ComponentLifeStage::Dead => panic!("Component {} is already dead!", self.id),
		}
	}
	
	/// Transitions `Alive` → `BeingRemoved`. Returns `false` if it was already being removed.
	pub fn mark_for_removal(&self) -> bool {
		match self.life_stage.replace(ComponentLifeStage::BeingRemoved) {
			ComponentLifeStage::New => panic!("Component {} has not been started yet!", self.id),
			ComponentLifeStage::Alive => true,
			ComponentLifeStage::BeingRemoved => false,
			ComponentLifeStage::Dead => panic!("Component {} is already dead!", self.id),
		}
	}
}

// Cloning inner creates new unique inner. It's unintuitive, but necessary to allow Clone Deriving in Components
impl Clone for ComponentInner {
	fn clone(&self) -> Self {
		Self::from_render_type(self.render_type)
	}
}

/// Weak, nullable reference to a component of type `C` (or `dyn Component`), stored as
/// `(entity id, component id)`.
///
/// Resolve it with `get`/`using`. Once the component no longer exists, the reference nulls itself.
pub struct ComponentRef<C: ?Sized> {
	inner: Cell<Option<(u64, u64)>>,
	phantom: PhantomData<C>,
}

impl<C: ?Sized + 'static> ComponentRef<C> {
	pub fn new(eid: u64, cid: u64) -> Self {
		ComponentRef {
			inner: Cell::new(Some((eid, cid))),
			phantom: PhantomData,
		}
	}
	
	pub fn null() -> Self {
		ComponentRef {
			inner: Cell::new(None),
			phantom: PhantomData,
		}
	}
	
	/// Replaces the referenced component.
	pub fn set(&self, other: impl Into<Self>) {
		self.inner.swap(&other.into().inner);
	}
	
	/// Resolves the reference. Returns `None` (and clears the reference) if the component or its
	/// entity was removed. Pending components are not returned, but the reference stays valid.
	pub fn get<'a>(&self, application: &'a Application) -> Option<&'a C> where C: Sized {
		if let Some((eid, _)) = self.inner.get() {
			if let Some(entity) = application.entity(eid) {
				return self.using(entity);
			} else if !application.pending_entity(eid) {
				self.inner.set(None);
			}
		}
		
		None
	}
	
	/// Like `get`, but returns the component as `dyn Component`.
	pub fn get_dyn<'a>(&self, application: &'a Application) -> Option<&'a dyn Component> {
		if let Some((eid, _)) = self.inner.get() {
			if let Some(entity) = application.entity(eid) {
				return self.using_dyn(entity);
			} else if !application.pending_entity(eid) {
				self.inner.set(None);
			}
		}
		
		None
	}
	
	/// Like `get`, but looks up the component in an already resolved entity.
	pub fn using<'e>(&self, entity: &'e Entity) -> Option<&'e C> where C: Sized {
		if let Some((_, cid)) = self.inner.get() {
			if let Some(component) = entity.component(cid) {
				return Some(component);
			} else if !entity.pending_component(cid) {
				self.inner.set(None);
			}
		}
		
		None
	}
	
	/// Like `using`, but returns the component as `dyn Component`.
	pub fn using_dyn<'e>(&self, entity: &'e Entity) -> Option<&'e dyn Component> {
		if let Some((_, cid)) = self.inner.get() {
			if let Some(component) = entity.component_dyn(cid) {
				return Some(component);
			} else if !entity.pending_component(cid) {
				self.inner.set(None);
			}
		}
		
		None
	}
	
	/// Returns a reference to the entity the component belongs to.
	pub fn entity(&self) -> EntityRef {
		match self.inner.get() {
			Some((eid, _)) => EntityRef::new(eid),
			None => EntityRef::null(),
		}
	}
	
	/// Returns the raw `(entity id, component id)` pair.
	pub fn inner(&self) -> Option<(u64, u64)> {
		self.inner.get()
	}
}

impl<C: ?Sized, D: ?Sized> PartialEq<ComponentRef<D>> for ComponentRef<C> {
	fn eq(&self, other: &ComponentRef<D>) -> bool {
		self.inner.eq(&other.inner)
	}
}

impl<C: ?Sized + 'static, D: AsRef<dyn Component>> PartialEq<D> for ComponentRef<C> {
	fn eq(&self, other: &D) -> bool {
		match self.inner() {
			None => false,
			Some((_, cid)) => cid.eq(&other.as_ref().inner().id)
		}
	}
}

impl<C: ?Sized> Debug for ComponentRef<C> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		if let Some((eid, cid)) = self.inner.get() {
			write!(f, "ref {}({}-{})", std::any::type_name::<C>(), eid, cid)
		} else {
			write!(f, "ref {}(null)", std::any::type_name::<C>())
		}
	}
}

impl<C: ?Sized> Clone for ComponentRef<C> {
	fn clone(&self) -> Self {
		ComponentRef {
			inner: self.inner.clone(),
			phantom: PhantomData,
		}
	}
}

impl<C: ?Sized + 'static> Default for ComponentRef<C> {
	fn default() -> Self {
		Self::null()
	}
}

impl<C: Component> From<ComponentRef<C>> for ComponentRef<dyn Component> {
	fn from(value: ComponentRef<C>) -> Self {
		ComponentRef {
			inner: value.inner.clone(),
			phantom: PhantomData,
		}
	}
}
