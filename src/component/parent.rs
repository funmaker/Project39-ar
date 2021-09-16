use std::time::Duration;

use crate::application::{Entity, Application, EntityRef};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::math::Isometry3;

#[derive(ComponentBase)]
pub struct Parent {
	#[inner] inner: ComponentInner,
	parent: EntityRef,
	offset: Isometry3,
}

impl Parent {
	pub fn new(parent: impl Into<EntityRef>, offset: Isometry3) -> Self {
		Parent {
			inner: ComponentInner::new(),
			parent: parent.into(),
			offset
		}
	}
}

impl Component for Parent {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		if let Some(parent) = self.parent.get(application) {
			entity.state_mut().position = parent.state().position * self.offset;
		} else {
			self.remove();
		}
		
		Ok(())
	}
}
