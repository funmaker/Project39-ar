use std::time::Duration;

use crate::application::{Entity, Application, EntityRef};
use crate::math::{AABB, Point3, Rot3, Vec3};
use super::{Component, ComponentBase, ComponentInner, ComponentError};


#[derive(ComponentBase)]
pub struct Seat {
	#[inner] inner: ComponentInner,
	trigger: AABB,
	driver: EntityRef,
}

impl Seat {
	pub fn new(trigger: AABB) -> Self {
		Seat {
			inner: ComponentInner::new_norender(),
			trigger,
			driver: EntityRef::null(),
		}
	}
}

impl Component for Seat {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		if let Some(hmd) = application.find_entity(|e| e.tag("Head").unwrap_or_default()) {
			let local_pos = entity.state().position.inverse() * *hmd.state().position * Point3::origin();
			
			if let Some(driver) = self.driver.get(application) {
				if !self.trigger.contains_local_point(&local_pos) || entity.has_tag("Grabbed") || driver.parent() != entity {
					driver.unset_tag("Seat");
					
					if driver.parent() == entity {
						driver.unset_parent(application);
					}
					
					let mut root_state = driver.state_mut();
					let mut forward = root_state.position.transform_vector(&vector!(0.0, 0.0, 1.0));
					forward.y = 0.0;
					root_state.position.rotation = Rot3::face_towards(&forward, &Vec3::y_axis());
					root_state.position.translation.y = 0.0;
 				}
			} else if let Some(root) = application.find_entity(|e| e.name == "VR Root") {
				if self.trigger.contains_local_point(&local_pos) && !entity.has_tag("Grabbed") && !root.has_tag("Seat") && root.parent().get(application).is_none() {
					self.driver.set(root.as_ref());
					root.set_parent(entity.as_ref(), true, application);
					root.set_tag("Seat", self.as_cref());
				}
			}
		}
		
		Ok(())
	}
}
