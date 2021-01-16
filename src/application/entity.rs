use std::sync::Arc;
use std::time::Duration;
use cgmath::{Vector3, Quaternion, Zero, Matrix4, Euler, Rad, Vector4};
use vulkano::command_buffer::AutoCommandBufferBuilder;

use crate::renderer::model::Model;
use crate::renderer::RendererRenderError;
use crate::utils::{decompose, mat4};
use crate::debug;
use openvr::TrackedDevicePose;

pub struct Entity {
	model: Arc<dyn Model>,
	pub position: Vector3<f32>,
	pub angle: Quaternion<f32>,
	pub velocity: Vector3<f32>,
	pub angular_velocity: Vector3<f32>,
}

impl Entity {
	pub fn new(model: Arc<dyn Model>, position: Vector3<f32>, angle: Quaternion<f32>) -> Self {
		Entity {
			model,
			position,
			angle,
			velocity: Vector3::zero(),
			angular_velocity: Vector3::new(0.0, 0.0, 0.0),
		}
	}
	
	pub fn tick(&mut self, delta_time: Duration) {
		self.position += self.velocity * delta_time.as_secs_f32();
		
		let ang_disp = self.angular_velocity * delta_time.as_secs_f32();
		let ang_disp = Euler::new(Rad(ang_disp.x), Rad(ang_disp.y), Rad(ang_disp.z));
		
		self.angle = self.angle * Quaternion::from(ang_disp);
	}
	
	pub fn render(&self, builder: &mut AutoCommandBufferBuilder, eye: u32) -> Result<(), RendererRenderError> {
		let model_matrix = Matrix4::from_translation(self.position)
		                 * Matrix4::from(self.angle);
		
		debug::draw_point(self.position, 10.0, Vector4::new(1.0, 0.0, 1.0, 1.0));
		
		self.model.render(builder, model_matrix, eye)
	}
	
	pub fn move_to_pose(&mut self, pose: TrackedDevicePose) {
		let orientation = decompose(mat4(pose.device_to_absolute_tracking()));
		
		self.position = orientation.disp;
		self.angle = orientation.rot;
		self.velocity = pose.velocity().clone().into();
		self.angular_velocity = pose.angular_velocity().clone().into();
	}
}
