use std::time::Duration;

use crate::application::{Entity, Application};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError, ComponentRef};
use crate::math::Isometry3;
use crate::component::parent::Parent;

#[derive(ComponentBase)]
pub struct ToolGun {
	#[inner] inner: ComponentInner,
	parent: ComponentRef<Parent>,
	offset: Isometry3,
}

impl ToolGun {
	pub fn new(offset: Isometry3) -> Self {
		ToolGun {
			inner: ComponentInner::new(),
			parent: ComponentRef::null(),
			offset,
		}
	}
}

impl Component for ToolGun {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		let state = entity.state();
		
		if self.parent.get(application).is_none() {
			let controller = application.find_entity(|e| e != entity && (e.name == "Controller" || e.name == "Hand") && (e.state().position.translation.vector - state.position.translation.vector).magnitude() < 0.1);
	
			if let Some(controller) = controller {
				self.parent.set(entity.add_component(Parent::new(controller, self.offset)));
				controller.state_mut().hidden = true;
			}
		}
		
		Ok(())
	}
	
	fn end(&self, _entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		if let Some(parent) = self.parent.entity().get(application) {
			parent.state_mut().hidden = false;
		}
		
		Ok(())
	}
}
