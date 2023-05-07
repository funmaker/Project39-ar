use std::time::Duration;

use crate::application::{Entity, Application};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError, ComponentRef};
use crate::component::parent::Parent;
use crate::math::{AABB, Point3, Rot3, Vec3};


#[derive(ComponentBase)]
pub struct Seat {
	#[inner] inner: ComponentInner,
	trigger: AABB,
	root_parent: ComponentRef<Parent>,
}

impl Seat {
	pub fn new(trigger: AABB) -> Self {
		Seat {
			inner: ComponentInner::new_norender(),
			trigger,
			root_parent: ComponentRef::null(),
		}
	}
}

impl Component for Seat {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		if let Some(hmd) = application.find_entity(|e| e.tag("Head").unwrap_or_default()) {
			let local_pos = entity.state().position.inverse() * *hmd.state().position * Point3::origin();
			
			if let Some(root_parent) = self.root_parent.get(application) {
				if !self.trigger.contains_local_point(&local_pos) || entity.has_tag("Grabbed") {
					let root = root_parent.entity(application);
					root.unset_tag("Seat");
					
					let mut root_state = root.state_mut();
					let mut forward = root_state.position.transform_vector(&vector!(0.0, 0.0, 1.0));
					forward.y = 0.0;
					root_state.position.rotation = Rot3::face_towards(&forward, &Vec3::y_axis());
					root_state.position.translation.y = 0.0;
					root_parent.remove();
				}
			} else if let Some(root) = application.find_entity(|e| e.name == "VR Root") {
				if self.trigger.contains_local_point(&local_pos) && !entity.has_tag("Grabbed") && !root.has_tag("Seat") {
					self.root_parent.set(root.add_component(Parent::new(entity.as_ref(), entity.state().position.inverse() * *root.state().position)));
					root.set_tag("Seat", self.as_cref());
				}
			}
		}
		
		Ok(())
	}
}
