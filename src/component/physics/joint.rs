use std::cell::Cell;
use egui::Ui;
use rapier3d::prelude::*;

use crate::application::{Entity, Application, Physics, EntityRef};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::utils::ExUi;

#[derive(ComponentBase)]
pub struct JointComponent {
	#[inner] inner: ComponentInner,
	template: GenericJoint,
	target: EntityRef,
	handle: Cell<ImpulseJointHandle>,
}

impl JointComponent {
	pub fn new(joint: impl Into<GenericJoint>, target: impl Into<EntityRef>) -> Self {
		JointComponent {
			inner: ComponentInner::new_norender(),
			template: joint.into(),
			target: target.into(),
			handle: Cell::new(ImpulseJointHandle::invalid()),
		}
	}
	
	pub fn inner<'p>(&self, physics: &'p Physics) -> &'p ImpulseJoint {
		physics.impulse_joint_set.get(self.handle.get()).unwrap()
	}
	
	pub fn inner_mut<'p>(&self, physics: &'p mut Physics) -> &'p mut ImpulseJoint {
		physics.impulse_joint_set.get_mut(self.handle.get()).unwrap()
	}
}

impl Component for JointComponent {
	fn start(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		let physics = &mut *application.physics.borrow_mut();
		
		if let Some(target) = self.target.get(application) {
			self.handle.set(physics.impulse_joint_set.insert(entity.rigid_body, target.rigid_body, self.template, true));
		}
		
		Ok(())
	}
	
	fn end(&self, _entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		let physics = &mut *application.physics.borrow_mut();
		
		physics.impulse_joint_set.remove(self.handle.get(), true);
		
		Ok(())
	}
	
	fn on_inspect_extra(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		ui.inspect_collapsing()
		  .title("Joint")
		  .show(ui, self.handle.get(), application);
	}
}
