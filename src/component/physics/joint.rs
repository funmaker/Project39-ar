use std::cell::Cell;
use rapier3d::prelude::*;

use crate::application::{Entity, Application, Physics, EntityRef};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};

#[derive(ComponentBase)]
pub struct JointComponent {
	#[inner] inner: ComponentInner,
	template: JointParams,
	target: EntityRef,
	handle: Cell<JointHandle>,
}

impl JointComponent {
	pub fn new(joint: impl Into<JointParams>, target: impl Into<EntityRef>) -> Self {
		JointComponent {
			inner: ComponentInner::new(),
			template: joint.into(),
			target: target.into(),
			handle: Cell::new(JointHandle::invalid()),
		}
	}
	
	pub fn inner<'p>(&self, physics: &'p Physics) -> &'p Joint {
		physics.joint_set.get(self.handle.get()).unwrap()
	}
	
	pub fn inner_mut<'p>(&self, physics: &'p mut Physics) -> &'p mut Joint {
		physics.joint_set.get_mut(self.handle.get()).unwrap()
	}
}

impl Component for JointComponent {
	fn start(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		let physics = &mut *application.physics.borrow_mut();
		
		if let Some(target) = self.target.get(application) {
			self.handle.set(physics.joint_set.insert(entity.rigid_body, target.rigid_body, self.template));
		}
		
		Ok(())
	}
	
	fn end(&self, _entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		let physics = &mut *application.physics.borrow_mut();
		
		physics.joint_set.remove(self.handle.get(), &mut physics.island_manager, &mut physics.rigid_body_set, true);
		
		Ok(())
	}
}
