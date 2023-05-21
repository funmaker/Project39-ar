use std::any::Any;
use std::cell::Cell;
use std::fmt::{Formatter, Debug};
use std::marker::PhantomData;
use std::time::Duration;
pub use project39_ar_derive::ComponentBase;
use egui::{Grid, Ui};
use enumflags2::BitFlags;

pub mod comedy;
pub mod example;
pub mod glow;
pub mod hand;
pub mod miku;
pub mod model;
pub mod parent;
pub mod pc_controlled;
pub mod physics;
pub mod pov;
pub mod seat;
pub mod srgb_test;
pub mod thruster;
pub mod toolgun;
pub mod vr;

use crate::application::{Application, Entity, EntityRef};
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::utils::{next_uid, IntoBoxed, ExUi};


pub type ComponentError = Box<dyn std::error::Error>;

pub trait ComponentBase: Any {
	fn inner(&self) -> &ComponentInner;
	fn inner_mut(&mut self) -> &mut ComponentInner;
	fn as_any(&self) -> &dyn Any;
	fn name(&self) -> &'static str;
	
	fn id(&self) -> u64 {
		self.inner().id
	}
	
	fn remove(&self) -> bool {
		self.inner().mark_for_removal()
	}
	
	fn entity<'a>(&self, application: &'a Application) -> &'a Entity {
		let eid = self.inner().entity_id.expect("Attempted to get entity of unmounted component");
		application.entity(eid).expect("Attempted to get entity of unmounted component")
	}
	
	fn as_cref(&self) -> ComponentRef<Self> where Self: Sized {
		ComponentRef::new(self.inner().entity_id.expect("Attempted to get reference of unmounted component"), self.inner().id)
	}
	
	fn as_cref_dyn(&self) -> ComponentRef<dyn Component> {
		ComponentRef::new(self.inner().entity_id.expect("Attempted to get reference of unmounted component"), self.inner().id)
	}
}

#[allow(unused_variables)]
pub trait Component: ComponentBase {
	fn start(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> { Ok(()) }
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> { Ok(()) }
	fn before_render(&self, entity: &Entity, context: &mut RenderContext, renderer: &mut Renderer) -> Result<(), ComponentError> { Ok(()) }
	fn render(&self, entity: &Entity, context: &mut RenderContext, renderer: &mut Renderer) -> Result<(), ComponentError> { Ok(()) }
	fn end(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> { Ok(()) }
	fn on_inspect(&self, entity: &Entity, ui: &mut Ui, application: &Application) {}
	fn on_inspect_extra(&self, entity: &Entity, ui: &mut Ui, application: &Application) {}
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

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum ComponentLifeStage {
	New,
	Alive,
	BeingRemoved,
	Dead,
}

#[derive(Debug)]
pub struct ComponentInner {
	id: u64,
	entity_id: Option<u64>,
	render_type: BitFlags<RenderType>,
	life_stage: Cell<ComponentLifeStage>,
}

impl ComponentInner {
	pub fn new_norender() -> Self {
		ComponentInner::from_render_type(BitFlags::empty())
	}
	
	pub fn from_render_type(render_type: impl Into<BitFlags<RenderType>>) -> Self {
		ComponentInner {
			id: next_uid(),
			entity_id: None,
			render_type: render_type.into(),
			life_stage: Cell::new(ComponentLifeStage::New),
		}
	}
	
	pub fn set_entity_id(&mut self, entity_id: u64) {
		assert!(self.entity_id.is_none(), "Component {} already mounted! Old: {} New: {}.", self.id, self.entity_id.unwrap(), entity_id);
		
		self.entity_id = Some(entity_id);
	}
	
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
	
	pub fn mark_started(&self) {
		match self.life_stage.replace(ComponentLifeStage::Alive) {
			ComponentLifeStage::New => {},
			ComponentLifeStage::Alive |
			ComponentLifeStage::BeingRemoved => panic!("Component {} in {:?} has already been started!", self.id, self.entity_id),
			ComponentLifeStage::Dead => panic!("Component {} is already dead!", self.id),
		}
	}
	
	pub fn mark_dead(&self) {
		match self.life_stage.replace(ComponentLifeStage::Dead) {
			ComponentLifeStage::New => panic!("Component {} has not been started yet!", self.id),
			ComponentLifeStage::Alive |
			ComponentLifeStage::BeingRemoved => {},
			ComponentLifeStage::Dead => panic!("Component {} is already dead!", self.id),
		}
	}
	
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
	
	pub fn set(&self, other: impl Into<Self>) {
		self.inner.swap(&other.into().inner);
	}
	
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
	
	pub fn entity(&self) -> EntityRef {
		match self.inner.get() {
			Some((eid, _)) => EntityRef::new(eid),
			None => EntityRef::null(),
		}
	}
	
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

impl<C: Component> From<ComponentRef<C>> for ComponentRef<dyn Component> {
	fn from(value: ComponentRef<C>) -> Self {
		ComponentRef {
			inner: value.inner.clone(),
			phantom: PhantomData,
		}
	}
}
