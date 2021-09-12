use std::time::Duration;

use crate::application::{Entity, Application};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};

#[derive(ComponentBase)]
pub struct PoV {
	#[inner] inner: ComponentInner,
}

impl PoV {
	pub fn new() -> Self {
		PoV {
			inner: ComponentInner::new(),
		}
	}
}

impl Component for PoV {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		application.camera_pos.set(entity.state().position);
		
		Ok(())
	}
}
