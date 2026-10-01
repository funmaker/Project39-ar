use std::cell::Cell;
use anyhow::Result;
use egui::Ui;
use rapier3d::prelude::*;

use crate::application::{Entity, Application};
use crate::utils::{ExUi, get_user_data};
use super::super::{Component, ComponentBase, ComponentInner};


#[derive(ComponentBase)]
pub struct ColliderComponent {
	#[inner] inner: ComponentInner,
	template: Collider,
	handle: Cell<ColliderHandle>,
}

impl ColliderComponent {
	pub fn new(collider: Collider) -> Self {
		ColliderComponent {
			inner: ComponentInner::new_norender(),
			template: collider,
			handle: Cell::new(ColliderHandle::invalid()),
		}
	}
	
	pub fn handle(&self) -> ColliderHandle {
		self.handle.get()
	}
	
	pub fn inner<'p>(&self, physics: &'p PhysicsWorld) -> &'p Collider {
		physics.colliders.get(self.handle.get()).unwrap()
	}
	
	pub fn inner_mut<'p>(&self, physics: &'p mut PhysicsWorld) -> &'p mut Collider {
		physics.colliders.get_mut(self.handle.get()).unwrap()
	}
}

impl Component for ColliderComponent {
	fn start(&self, entity: &Entity, application: &Application) -> Result<()> {
		let physics = &mut *application.physics.borrow_mut();
		
		let mut collider = self.template.clone();
		collider.user_data = get_user_data(entity.id, self.id());
		self.handle.set(physics.insert_collider(collider, Some(entity.rigid_body)));
		
		Ok(())
	}
	
	fn end(&self, _entity: &Entity, application: &Application) -> Result<()> {
		let physics = &mut *application.physics.borrow_mut();
		
		physics.remove_collider(self.handle.get());
		
		Ok(())
	}
	
	fn on_inspect_extra(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		ui.inspect_collapsing()
		  .title("Collider")
		  .show(ui, self.handle.get(), application);
	}
}

impl Into<ColliderComponent> for Collider {
	fn into(self) -> ColliderComponent {
		ColliderComponent::new(self)
	}
}
