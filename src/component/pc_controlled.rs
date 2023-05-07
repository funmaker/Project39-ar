use std::time::Duration;

use crate::application::{Entity, Application, Key};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::math::{Vec3, Isometry3, PI, to_euler, from_euler};


#[derive(ComponentBase)]
pub struct PCControlled {
	#[inner] inner: ComponentInner,
}

impl PCControlled {
	pub fn new() -> Self {
		PCControlled {
			inner: ComponentInner::new_norender(),
		}
	}
}

impl Component for PCControlled {
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> {
		let mut entity = entity.state_mut();
		let mut position = entity.position.translation.vector;
		let (mut pitch, mut yaw, mut roll) = to_euler(entity.position.rotation);
		
		let get_key = |key: Key| application.input.keyboard.pressed(key) as i32 as f32;
		
		let x = get_key(Key::D) - get_key(Key::A);
		let y = get_key(Key::Space) - get_key(Key::LControl);
		let z = get_key(Key::S) - get_key(Key::W);
		let rr = get_key(Key::Numpad7) - get_key(Key::Numpad9);
		let dist = (0.5 + get_key(Key::LShift) * 1.0) * delta_time.as_secs_f32();
		let mouse_x = application.input.mouse.axis(0);
		let mouse_y = application.input.mouse.axis(1);
		
		yaw = yaw + -mouse_x * 0.01;
		pitch = (pitch + -mouse_y * 0.01).clamp(-PI / 2.0, PI / 2.0);
		roll = roll + rr * 0.1;
		
		let rot = from_euler(pitch, yaw, roll);
		position += rot * vector!(x, 0.0, z) * dist + Vec3::y() * y * dist;
		
		*entity.position = Isometry3::from_parts(position.into(), rot);
		
		Ok(())
	}
}
