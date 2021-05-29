use std::time::Duration;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
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
	pub morphs: Vec<f32>,
	model: Box<dyn Model>,
	hair_swing: f32,
}

impl Entity {
	pub fn new(name: impl Into<String>, model: impl IntoBoxedModel, position: Point3, angle: Rot3) -> Self {
		let model = model.into();
		
		Entity {
			name: name.into(),
			position: Isometry3::from_parts(position.coords.into(), angle),
			velocity: Vec3::zeros(),
			angular_velocity: Vec3::zeros(),
			bones: model.get_default_bones().to_vec(),
			morphs: vec![0.0; model.morphs_count()],
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
				self.bones[id].anim_transform.isometry.rotation = Rot3::from_euler_angles(0.0, 0.0, swing);
			}
			if self.bones[id].name == "Bend" {
				let swing = (self.hair_swing / 3.0).sin() * std::f32::consts::PI / 2.0;
				self.bones[id].anim_transform.isometry.rotation = Rot3::from_euler_angles(0.0, 0.0, swing);
			}
		}
		
		if self.name == "初音ミク" {
			let presets = vec![
				(1, &[0, 29, 66][..]),
				(2, &[1, 45, 92]),
				(3, &[24, 65]),
				(4, &[39, 60]),
				(5, &[47, 2, 61]),
			];
			
			for morph in self.morphs.iter_mut() {
				*morph = (*morph - 5.0 * delta_time.as_secs_f32()).clamp(0.0, 1.0);
			}
			
			let active = presets.iter().filter(|p| debug::get_flag_or_default(&format!("KeyKey{}", p.0))).flat_map(|p| p.1.iter());
			
			for &id in active {
				self.morphs[id] = (self.morphs[id] + 10.0 * delta_time.as_secs_f32()).clamp(0.0, 1.0);
			}
		}
	}
	
	pub fn pre_render(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), RendererRenderError> {
		self.model.pre_render(builder, &self.position.to_transform(), &self.bones, &self.morphs)?;
		
		Ok(())
	}
	
	pub fn render(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), RendererRenderError> {
		let pos: Point3 = self.position.translation.vector.into();
		let ang = &self.position.rotation;
		
		debug::draw_point(&pos, 32.0, Color::magenta());
		debug::draw_line(&pos, &pos + ang * Vec3::x() * 0.3, 4.0, Color::red());
		debug::draw_line(&pos, &pos + ang * Vec3::y() * 0.3, 4.0, Color::green());
		debug::draw_line(&pos, &pos + ang * Vec3::z() * 0.3, 4.0, Color::blue());
		debug::draw_text(&self.name, &pos, debug::DebugOffset::bottom_right(32.0, 32.0), 128.0, Color::magenta());
		
		self.model.render(builder, &self.position.to_transform())?;
		
		Ok(())
	}
	
	pub fn move_to_pose(&mut self, pose: TrackedDevicePose) {
		let orientation: AMat4 = pose.device_to_absolute_tracking().to_transform();
		let orientation: Similarity3 = orientation.to_subset().unwrap();
		
		self.position = orientation.isometry;
		self.velocity = pose.velocity().clone().into();
		self.angular_velocity = pose.angular_velocity().clone().into();
	}
}

pub trait IntoBoxedModel {
	fn into(self) -> Box<dyn Model>;
}

impl IntoBoxedModel for Box<dyn Model> {
	fn into(self) -> Box<dyn Model> {
		self
	}
}

impl<M: Model + 'static> IntoBoxedModel for M {
	fn into(self) -> Box<dyn Model> {
		Box::new(self)
	}
}
