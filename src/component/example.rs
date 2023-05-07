use std::time::Duration;

use crate::application::{Entity, Application};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};


#[derive(ComponentBase)]
pub struct Example {
	#[inner] inner: ComponentInner,
}

impl Example {
	pub fn new() -> Self {
		Example {
			inner: ComponentInner::new_norender(),
		}
	}
}

impl Component for Example {
	fn tick(&self, _entity: &Entity, _application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		
		Ok(())
	}
}
