use std::cell::Cell;
use rapier3d::prelude::*;

use crate::application::{Entity, Application, Physics};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::utils::get_userdata;

#[derive(ComponentBase)]
pub struct ColliderComponent {
	#[inner] inner: ComponentInner,
	template: Collider,
	handle: Cell<ColliderHandle>,
}

impl ColliderComponent {
	pub fn new(collider: Collider) -> Self {
		ColliderComponent {
			inner: ComponentInner::new(),
			template: collider,
			handle: Cell::new(ColliderHandle::invalid()),
		}
	}
	
	pub fn inner<'p>(&self, physics: &'p Physics) -> &'p Collider {
		physics.collider_set.get(self.handle.get()).unwrap()
	}
	
	pub fn inner_mut<'p>(&self, physics: &'p mut Physics) -> &'p mut Collider {
		physics.collider_set.get_mut(self.handle.get()).unwrap()
	}
}

impl Component for ColliderComponent {
	fn start(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		let physics = &mut *application.physics.borrow_mut();
		
		let mut collider = self.template.clone();
		collider.user_data = get_userdata(self.inner.entity_id.unwrap(), self.inner.id);
		self.handle.set(physics.collider_set.insert_with_parent(collider, entity.rigid_body, &mut physics.rigid_body_set));
		
		Ok(())
	}
	
	fn end(&self, _entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		let physics = &mut *application.physics.borrow_mut();
		
		physics.collider_set.remove(self.handle.get(), &mut physics.island_manager, &mut physics.rigid_body_set, true);
		
		Ok(())
	}
}

impl Into<ColliderComponent> for Collider {
	fn into(self) -> ColliderComponent {
		ColliderComponent::new(self)
	}
}
