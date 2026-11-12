use std::time::Duration;
use anyhow::Result;

use crate::application::{Entity, Application};
use super::{Component, ComponentBase, ComponentInner};


/// Minimal component template. Copy this file when creating a new component.
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
	fn tick(&self, _entity: &Entity, _application: &Application, _delta_time: Duration) -> Result<()> {
		
		Ok(())
	}
}
