use std::time::Duration;
use std::cell::Cell;

use crate::application::{Entity, Application};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::math::{Rot3, Vec3, Isometry3};
use crate::debug;

#[derive(ComponentBase)]
pub struct PCControlled {
	#[inner] inner: ComponentInner,
	rotation: Cell<(f32, f32)>,
}

impl PCControlled {
	pub fn new() -> Self {
		PCControlled {
			inner: ComponentInner::new(),
			rotation: Cell::new((0.0, 0.0)),
		}
	}
}

impl Component for PCControlled {
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> {
		let mut entity = entity.state_mut();
		let mut position = entity.position.translation.vector;
		let (mut pitch, mut yaw) = self.rotation.get();
		
		fn get_key(key: &str) -> f32 {
			debug::get_flag_or_default::<bool>(key) as i32 as f32
		}
		
		let x = get_key("KeyD") - get_key("KeyA");
		let y = get_key("KeySpace") - get_key("KeyCtrl");
		let z = get_key("KeyS") - get_key("KeyW");
		let dist = (0.5 + get_key("KeyLShift") * 1.0) * delta_time.as_secs_f32();
		let mouse_move = debug::get_flag("mouse_move").unwrap_or((0.0_f32, 0.0_f32));
		debug::set_flag("mouse_move", (0.0_f32, 0.0_f32));
		
		yaw = yaw + -mouse_move.0 * 0.01;
		pitch = (pitch + -mouse_move.1 * 0.01).clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
		
		let rot = Rot3::from_euler_angles(pitch, yaw, 0.0);
		position += rot * Vec3::new(x, 0.0, z) * dist + Vec3::y() * y * dist;
		
		self.rotation.set((pitch, yaw));
		entity.position = Isometry3::from_parts(position.into(), rot);
		application.camera_pos.set(entity.position);
		
		Ok(())
	}
}
