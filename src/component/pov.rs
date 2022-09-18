use std::cell::Cell;
use std::time::Duration;

use crate::application::{Entity, Application, Key, EntityRef};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::component::parent::Parent;
use crate::component::pc_controlled::PCControlled;
use crate::math::Isometry3;

#[derive(ComponentBase)]
pub struct PoV {
	#[inner] inner: ComponentInner,
	detachable: bool,
	detached: EntityRef,
}

impl PoV {
	pub fn new(detachable: bool) -> Self {
		PoV {
			inner: ComponentInner::new_norender(),
			detachable,
			detached: EntityRef::null(),
		}
	}
}

impl Component for PoV {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		application.pov.set(entity.as_ref());
		
		if self.detachable && application.input.keyboard.down(Key::Back) {
			if let Some(detached) = self.detached.get(application) {
				detached.remove();
			} else {
				self.detached.set(application.add_entity(
					Entity::builder("Detached PoV")
					       .component(DetachedPoV::new())
					       .component(Parent::new(entity, Isometry3::identity()))
					       .build()
				));
			}
		}
		
		Ok(())
	}
}

#[derive(ComponentBase)]
pub struct DetachedPoV {
	#[inner] inner: ComponentInner,
	free: Cell<bool>,
}

impl DetachedPoV {
	pub fn new() -> Self {
		DetachedPoV {
			inner: ComponentInner::new_norender(),
			free: Cell::new(false),
		}
	}
}

impl Component for DetachedPoV {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		application.detached_pov.set(entity.as_ref());
		
		if !self.free.get() && (
			application.input.keyboard.down(Key::W) ||
			application.input.keyboard.down(Key::S) ||
			application.input.keyboard.down(Key::A) ||
			application.input.keyboard.down(Key::D)
		) {
			if let Some(parent) = entity.find_component_by_type::<Parent>() {
				parent.remove();
			}
			
			entity.add_component(PCControlled::new());
			
			self.free.set(true);
		}
		
		Ok(())
	}
}
