use std::cell::Cell;
use std::time::Duration;
use egui::Ui;

use crate::application::{Entity, Application, EntityRef};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::math::{Isometry3, Vec3};
use crate::utils::ExUi;

#[derive(ComponentBase)]
pub struct Parent {
	#[inner] inner: ComponentInner,
	pub target: EntityRef,
	pub offset: Cell<Isometry3>,
}

impl Parent {
	pub fn new(target: impl Into<EntityRef>, offset: Isometry3) -> Self {
		Parent {
			inner: ComponentInner::new_norender(),
			target: target.into(),
			offset: Cell::new(offset),
		}
	}
}

impl Component for Parent {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		if let Some(parent) = self.target.get(application) {
			let mut state = entity.state_mut();
			*state.position = *parent.state().position * self.offset.get();
			*state.velocity = Vec3::zeros();
			*state.angular_velocity = Vec3::zeros();
		} else {
			self.remove();
		}
		
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		ui.inspect_row("Parent", &self.target, application);
		ui.inspect_row("Offset", &self.offset, ());
	}
}
