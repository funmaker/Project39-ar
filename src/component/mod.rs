use std::any::Any;
use std::cell::Cell;
use std::time::Duration;
use std::marker::PhantomData;
use std::fmt::{Formatter, Debug};
use enumflags2::BitFlags;
pub use project39_ar_derive::ComponentBase;

pub mod example;
pub mod model;
pub mod miku;
pub mod vr;
pub mod pov;
pub mod pc_controlled;
pub mod toolgun;
pub mod parent;
pub mod physics;
pub mod glow;
pub mod hand;
pub mod seat;
pub mod thruster;
pub mod comedy;

use crate::application::{Application, Entity, EntityRef};
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::utils::{next_uid, IntoBoxed};

pub type ComponentError = Box<dyn std::error::Error>;

pub trait ComponentBase: Any {
	fn inner(&self) -> &ComponentInner;
	fn inner_mut(&mut self) -> &mut ComponentInner;
	fn as_any(&self) -> &dyn Any;
	
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
}

#[allow(unused_variables)]
pub trait Component: ComponentBase {
	fn start(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> { Ok(()) }
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> { Ok(()) }
	fn before_render(&self, entity: &Entity, context: &mut RenderContext, renderer: &mut Renderer) -> Result<(), ComponentError> { Ok(()) }
	fn render(&self, entity: &Entity, context: &mut RenderContext, renderer: &mut Renderer) -> Result<(), ComponentError> { Ok(()) }
	fn end(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> { Ok(()) }
	
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

pub struct ComponentRef<C> {
	inner: Cell<Option<(u64, u64)>>,
	phantom: PhantomData<C>,
}

impl<C: 'static> ComponentRef<C> {
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
	
	pub fn set(&self, other: Self) {
		self.inner.swap(&other.inner);
	}
	
	pub fn get<'a>(&self, application: &'a Application) -> Option<&'a C> {
		if let Some((eid, cid)) = self.inner.get() {
			if let Some(component) = application.entity(eid)
			                                    .and_then(|e| e.component(cid)) {
				Some(component)
			} else {
				self.inner.set(None);
				None
			}
		} else {
			None
		}
	}
	
	pub fn using<'e>(&self, entity: &'e Entity) -> Option<&'e C> {
		if let Some((_, cid)) = self.inner.get() {
			if let Some(component) = entity.component(cid) {
				Some(component)
			} else {
				self.inner.set(None);
				None
			}
		} else {
			None
		}
	}
	
	pub fn entity(&self) -> EntityRef {
		match self.inner.get() {
			Some((eid, _)) => EntityRef::new(eid),
			None => EntityRef::null(),
		}
	}
}

impl<C, D> PartialEq<ComponentRef<D>> for ComponentRef<C> {
	fn eq(&self, other: &ComponentRef<D>) -> bool {
		self.inner.eq(&other.inner)
	}
}

impl<C> Debug for ComponentRef<C> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		if let Some((eid, cid)) = self.inner.get() {
			write!(f, "ref {}({}-{})", std::any::type_name::<C>(), eid, cid)
		} else {
			write!(f, "ref {}(null)", std::any::type_name::<C>())
		}
	}
}

impl<C> Clone for ComponentRef<C> {
	fn clone(&self) -> Self {
		ComponentRef {
			inner: self.inner.clone(),
			phantom: PhantomData,
		}
	}
}
