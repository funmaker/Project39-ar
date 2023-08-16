use std::cell::Cell;
use std::time::Duration;
use egui::Ui;

use crate::application::{Entity, Application};
use crate::math::{Isometry3, Translation3};
use crate::utils::ExUi;
use super::{Component, ComponentBase, ComponentInner, ComponentError};


#[derive(ComponentBase)]
pub struct TestComponent {
	#[inner] inner: ComponentInner,
	phys: bool,
	orig: Cell<Isometry3>,
	time: Cell<f32>,
	running: Cell<bool>,
	speed: Cell<f32>,
}

impl TestComponent {
	pub fn new(phys: bool, speed: f32) -> Self {
		TestComponent {
			inner: ComponentInner::new_norender(),
			phys,
			orig: Cell::new(Isometry3::identity()),
			time: Cell::new(0.0),
			running: Cell::new(true),
			speed: Cell::new(speed),
		}
	}
}

impl Component for TestComponent {
	fn start(&self, entity: &Entity, _application: &Application) -> Result<(), ComponentError> {
		self.orig.set(*entity.state().position);
		Ok(())
	}
	
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> {
		if !self.running.get() { return Ok(()) }
		
		self.time.set(self.time.get() + delta_time.as_secs_f32());
		let time = self.time.get();
		let position = self.orig.get() * Translation3::new(0.0, 0.0, ((time * self.speed.get().abs()) % 1.0 - 0.5).abs() * self.speed.get().signum());
		
		if self.phys {
			entity.rigid_body_mut(&mut *application.physics.borrow_mut()).set_position(position, true);
		} else {
			*entity.state_mut().position = position;
		}
		
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, _application: &Application) {
		ui.inspect_row("Speed", &self.speed, (0.1, -2.0..=2.0));
	}
	
	fn on_inspect_extra(&self, _entity: &Entity, ui: &mut Ui, _application: &Application) {
		if self.running.get() {
			if ui.button("Stop").clicked() {
				self.running.set(false);
			}
		} else {
			if ui.button("Start").clicked() {
				self.running.set(true);
			}
		}
	}
}
