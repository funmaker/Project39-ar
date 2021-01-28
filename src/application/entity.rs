use std::sync::Arc;
use std::time::Duration;
use cgmath::{Vector3, Quaternion, Zero, Matrix4, Euler, Rad, Vector4};
use vulkano::command_buffer::AutoCommandBufferBuilder;

mod bone;

use crate::renderer::model::Model;
use crate::renderer::RendererRenderError;
use crate::utils::{decompose, mat4};
use crate::debug;
pub use bone::{Bone, BoneConnection};
use openvr::TrackedDevicePose;

pub struct Entity {
	model: Arc<dyn Model>,
	pub name: String,
	pub position: Vector3<f32>,
	pub angle: Quaternion<f32>,
	pub velocity: Vector3<f32>,
	pub angular_velocity: Vector3<f32>,
	pub bones: Vec<Bone>,
	pub hair_swing: f32,
}

impl Entity {
	pub fn new(name: impl Into<String>, model: impl IntoArcModel, position: Vector3<f32>, angle: Quaternion<f32>) -> Self {
		let model = model.into();
		
		Entity {
			name: name.into(),
			position,
			angle,
			velocity: Vector3::zero(),
			angular_velocity: Vector3::new(0.0, 0.0, 0.0),
			bones: model.get_default_bones(),
			model,
			hair_swing: 0.0,
		}
	}
	
	pub fn tick(&mut self, delta_time: Duration) {
		self.position += self.velocity * delta_time.as_secs_f32();
		
		let ang_disp = self.angular_velocity * delta_time.as_secs_f32();
		let ang_disp = Euler::new(Rad(ang_disp.x), Rad(ang_disp.y), Rad(ang_disp.z));
		
		self.angle = self.angle * Quaternion::from(ang_disp);
		
		self.hair_swing += delta_time.as_secs_f32() * 3.0;
		
		let swing = self.hair_swing.sin() / 30.0;
		
		for id in 0..self.bones.len() {
			if self.bones[id].name.starts_with("Right H") || self.bones[id].name.starts_with("Left H") {
				self.bones[id].transform = Matrix4::from_translation(self.bones[id].orig - self.bones[self.bones[id].parent.unwrap()].orig) * Matrix4::from_angle_z(Rad(swing));
			}
			if self.bones[id].name == "Bend" {
				self.bones[id].transform = Matrix4::from_translation(self.bones[id].orig - self.bones[self.bones[id].parent.unwrap()].orig) * Matrix4::from_angle_z(Rad((self.hair_swing / 3.0).sin() * std::f32::consts::PI / 4.0));
			}
		}
	}
	
	pub fn render(&self, builder: &mut AutoCommandBufferBuilder, eye: u32) -> Result<(), RendererRenderError> {
		let model_matrix = Matrix4::from_translation(self.position)
		                 * Matrix4::from(self.angle);
		
		debug::draw_point(self.position, 32.0, Vector4::new(1.0, 0.0, 1.0, 1.0));
		debug::draw_line(self.position, self.position + self.angle * Vector3::unit_x() * 0.3, 4.0, Vector4::new(1.0, 0.0, 0.0, 1.0));
		debug::draw_line(self.position, self.position + self.angle * Vector3::unit_y() * 0.3, 4.0, Vector4::new(0.0, 1.0, 0.0, 1.0));
		debug::draw_line(self.position, self.position + self.angle * Vector3::unit_z() * 0.3, 4.0, Vector4::new(0.0, 0.0, 1.0, 1.0));
		debug::draw_text(&self.name, self.position, debug::DebugOffset::bottom_right(32.0, 32.0), 128.0, Vector4::new(1.0, 0.0, 1.0, 1.0));
		
		self.model.render(builder, model_matrix, eye, &self.bones)
	}
	
	pub fn move_to_pose(&mut self, pose: TrackedDevicePose) {
		let orientation = decompose(mat4(pose.device_to_absolute_tracking()));
		
		self.position = orientation.disp;
		self.angle = orientation.rot;
		self.velocity = pose.velocity().clone().into();
		self.angular_velocity = pose.angular_velocity().clone().into();
	}
}

pub trait IntoArcModel {
	fn into(self) -> Arc<dyn Model>;
}

impl IntoArcModel for Arc<dyn Model> {
	fn into(self) -> Arc<dyn Model> {
		self
	}
}

impl<M: Model + 'static> IntoArcModel for Arc<M> {
	fn into(self) -> Arc<dyn Model> {
		self
	}
}

impl<M: Model + 'static> IntoArcModel for M {
	fn into(self) -> Arc<dyn Model> {
		Arc::new(self)
	}
}
