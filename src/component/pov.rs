use std::cell::Cell;
use std::time::Duration;
use egui::Ui;

use crate::application::{Entity, Application, Key, EntityRef};
use crate::utils::ExUi;
use super::{Component, ComponentBase, ComponentInner, ComponentError};
use super::model::simple::ObjAsset;
use super::pc_controlled::PCControlled;


#[derive(ComponentBase)]
pub struct PoV {
	#[inner] inner: ComponentInner,
	detachable: bool,
	detached: EntityRef,
}

impl PoV {
	pub fn new(detachable: bool) -> Self {
		PoV {
			inner: ComponentInner::new_norender(),
			detachable,
			detached: EntityRef::null(),
		}
	}
}

impl Component for PoV {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		application.pov.set(entity.as_ref());
		
		if self.detachable && application.input.keyboard.down(Key::Back) {
			if let Some(detached) = self.detached.get(application) {
				detached.remove();
			} else {
				self.detached.set(application.add_entity(
					Entity::builder("Detached PoV")
					       .position(*entity.state().position)
					       .parent(entity.as_ref(), true)
					       .component(application.renderer.borrow_mut().load(ObjAsset::at("camera/camera.obj", "camera/camera.png"))?)
					       .component(DetachedPoV::new())
					       .collider_from_aabb(1000.0)
					       .build()
				));
			}
		}
		
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		ui.inspect_row("Detached Ent", &self.detached, application);
	}
	
	fn on_inspect_extra(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		if let Some(detached) = self.detached.get(application) {
			if ui.button("Reattach").clicked() {
				detached.remove();
			}
		}
	}
}

#[derive(ComponentBase)]
pub struct DetachedPoV {
	#[inner] inner: ComponentInner,
	free: Cell<bool>,
}

impl DetachedPoV {
	pub fn new() -> Self {
		DetachedPoV {
			inner: ComponentInner::new_norender(),
			free: Cell::new(false),
		}
	}
}

impl Component for DetachedPoV {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		application.detached_pov.set(entity.as_ref());
		
		if entity.parent().get(application).is_some() && (
			application.input.keyboard.down(Key::W) ||
			application.input.keyboard.down(Key::S) ||
			application.input.keyboard.down(Key::A) ||
			application.input.keyboard.down(Key::D)
		) {
			entity.unset_parent(application);
			entity.add_component(PCControlled::new());
		}
		
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, _application: &Application) {
		ui.inspect_row("Free Cam", &self.free, ());
	}
	
	fn on_inspect_extra(&self, entity: &Entity, ui: &mut Ui, _application: &Application) {
		if ui.button("Reattach").clicked() {
			entity.remove();
		}
	}
}
