use std::time::Duration;

use crate::application::{Entity, Application};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError, ComponentRef};
use crate::math::{AABB, Point3, Color, Rot3, Vec3};
use crate::component::parent::Parent;
use crate::debug;

#[derive(ComponentBase)]
pub struct Seat {
	#[inner] inner: ComponentInner,
	trigger: AABB,
	root_parent: ComponentRef<Parent>,
}

impl Seat {
	pub fn new(trigger: AABB) -> Self {
		Seat {
			inner: ComponentInner::new(),
			trigger,
			root_parent: ComponentRef::null(),
		}
	}
}

impl Component for Seat {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		if let Some(hmd) = application.find_entity(|e| e.tag("Head").unwrap_or_default()) {
			let local_pos = entity.state().position.inverse() * hmd.state().position * Point3::origin();
			
			if let Some(root_parent) = self.root_parent.get(application) {
				if !self.trigger.contains_local_point(&local_pos) || entity.has_tag("Grabbed") {
					println!("Left");
					
					let mut root = root_parent.entity(application).state_mut();
					let mut forward = root.position.transform_vector(&vector!(0.0, 0.0, 1.0));
					forward.y = 0.0;
					root.position.rotation = Rot3::face_towards(&forward, &Vec3::y_axis());
					root.position.translation.y = 0.0;
					root_parent.remove();
				}
			} else if let Some(root) = application.find_entity(|e| e.name == "VR Root") {
				if self.trigger.contains_local_point(&local_pos) && !entity.has_tag("Grabbed") {
					println!("Entered");
					self.root_parent.set(root.add_component(Parent::new(entity.as_ref(), entity.state().position.inverse() * root.state().position)))
				}
			}
		}
		
		Ok(())
	}
}
