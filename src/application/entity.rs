use std::sync::Arc;
use std::time::Duration;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use openvr::TrackedDevicePose;
use simba::scalar::SupersetOf;

mod bone;

use crate::renderer::model::Model;
use crate::renderer::RendererRenderError;
use crate::math::{Vec3, Rot3, Point3, Isometry3, Color, ToTransform, AMat4, Similarity3};
use crate::debug;
pub use bone::{Bone, BoneConnection};

pub struct Entity {
	pub name: String,
	pub position: Isometry3,
	pub velocity: Vec3,
	pub angular_velocity: Vec3,
	pub bones: Vec<Bone>,
	model: Arc<dyn Model>,
	hair_swing: f32,
}

impl Entity {
	pub fn new(name: impl Into<String>, model: impl IntoArcModel, position: Point3, angle: Rot3) -> Self {
		let model = model.into();
		
		Entity {
			name: name.into(),
			position: Isometry3::from_parts(position.coords.into(), angle),
			velocity: Vec3::zeros(),
			angular_velocity: Vec3::zeros(),
			bones: model.get_default_bones(),
			model,
			hair_swing: 0.0,
		}
	}
	
	pub fn tick(&mut self, delta_time: Duration) {
		let ang_disp = &self.angular_velocity * delta_time.as_secs_f32();
		let (pitch, yaw, roll) = (ang_disp.x, ang_disp.y, ang_disp.z);
		
		self.position.translation.vector += &self.velocity * delta_time.as_secs_f32();
		self.position.rotation *= Rot3::from_euler_angles(roll, pitch, yaw);
		
		self.hair_swing += delta_time.as_secs_f32() * 3.0;
		
		for id in 0..self.bones.len() {
			if self.bones[id].name.starts_with("Right H") || self.bones[id].name.starts_with("Left H") {
				let swing = self.hair_swing.sin() / 30.0;
				self.bones[id].transform = Similarity3::new(&self.bones[id].orig - &self.bones[self.bones[id].parent.unwrap()].orig, Vec3::z() * swing, 1.0);
			}
			if self.bones[id].name == "Bend" {
				let swing = (self.hair_swing / 3.0).sin() * std::f32::consts::PI / 4.0;
				self.bones[id].transform = Similarity3::new(&self.bones[id].orig - &self.bones[self.bones[id].parent.unwrap()].orig, Vec3::z() * swing, 1.0);
			}
		}
	}
	
	pub fn render(&self, builder: &mut AutoCommandBufferBuilder, eye: u32) -> Result<(), RendererRenderError> {
		let pos: Point3 = self.position.translation.vector.into();
		let ang = &self.position.rotation;
		
		debug::draw_point(&pos, 32.0, Color::magenta());
		debug::draw_line(&pos, &pos + ang * Vec3::x() * 0.3, 4.0, Color::red());
		debug::draw_line(&pos, &pos + ang * Vec3::y() * 0.3, 4.0, Color::green());
		debug::draw_line(&pos, &pos + ang * Vec3::z() * 0.3, 4.0, Color::blue());
		debug::draw_text(&self.name, &pos, debug::DebugOffset::bottom_right(32.0, 32.0), 128.0, Color::magenta());
		
		self.model.render(builder, &self.position.to_transform(), eye, &self.bones)
	}
	
	pub fn move_to_pose(&mut self, pose: TrackedDevicePose) {
		let orientation: AMat4 = pose.device_to_absolute_tracking().to_transform();
		let orientation: Similarity3 = orientation.to_subset().unwrap();
		
		self.position = orientation.isometry;
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
