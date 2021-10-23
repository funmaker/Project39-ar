use std::time::Duration;

use crate::application::{Entity, Application, EntityRef};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::math::{Point3, Color};
use crate::debug;

#[derive(ComponentBase)]
pub struct Rope {
	#[inner] inner: ComponentInner,
	local_offset: Point3,
	other: EntityRef,
	other_offset: Point3,
	length: f32,
	strength: f32,
}

impl Rope {
	pub fn new(local_offset: Point3, other: EntityRef, other_offset: Point3, length: f32, strength: f32) -> Self {
		Rope {
			inner: ComponentInner::new(),
			local_offset,
			other,
			other_offset,
			length,
			strength,
		}
	}
}

impl Component for Rope {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		let other = match self.other.get(application) {
			Some(other) => other,
			None => {
				self.remove();
				return Ok(());
			}
		};
		
		let self_pos = entity.state().position * self.local_offset;
		let other_pos = other.state().position * self.other_offset;
		let offset = self_pos - other_pos;
		let magnitude = offset.magnitude();
		
		if magnitude > self.length {
			let mut physics = application.physics.borrow_mut();
			let force = (magnitude - self.length) * self.strength;
			
			physics.rigid_body_set.get_mut(entity.rigid_body).unwrap().apply_force(offset.normalize() * -force, /*self_pos,*/ true);
			physics.rigid_body_set.get_mut(other.rigid_body).unwrap().apply_force(offset.normalize() * force, /*other_pos,*/ true);
		}
		
		debug::draw_line(&self_pos, &other_pos, 8.0, Color::dblack());
		
		Ok(())
	}
}
