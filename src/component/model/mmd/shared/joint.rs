use crate::math::{Isometry3, Vec2, Vec3};
use super::super::BodyPart;


pub type JointType = mmd::pmx::joint::JointType;

#[derive(Clone, Debug)]
pub struct JointDesc {
	pub name: String,
	pub joint_type: JointType,
	pub collider_a: usize,
	pub collider_b: usize,
	pub position: Isometry3,
	pub position_min: Vec3,
	pub position_max: Vec3,
	pub rotation_min: Vec3,
	pub rotation_max: Vec3,
	pub position_spring: Vec3,
	pub rotation_spring: Vec3,
	pub body_part: Option<BodyPart>,
	pub cone_twist: Option<ConeTwistDesc>,
}

impl JointDesc {
	pub fn new(name: impl Into<String>,
	           joint_type: JointType,
	           collider_a: usize,
	           collider_b: usize,
	           position: Isometry3,
	           position_min: Vec3,
	           position_max: Vec3,
	           rotation_min: Vec3,
	           rotation_max: Vec3,
	           position_spring: Vec3,
	           rotation_spring: Vec3,
	           body_part: Option<BodyPart>,
	           cone_twist: Option<ConeTwistDesc>) -> Self {
		JointDesc {
			name: name.into(),
			joint_type,
			collider_a,
			collider_b,
			position,
			position_min,
			position_max,
			rotation_min,
			rotation_max,
			position_spring,
			rotation_spring,
			body_part,
			cone_twist,
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct ConeTwistDesc {
	pub swing: f32,
	pub twist: (f32, f32),
	pub bias: Vec2,
}

impl ConeTwistDesc {
	pub fn from_rotation_limits(min: Vec3, max: Vec3) -> Self {
		let swing = ((max.x - min.x) + (max.y - min.y)) / 4.0;
		ConeTwistDesc {
			swing,
			twist: (min.z, max.z),
			bias: vector![
				(min.x + max.x) / 2.0,
				(min.y + max.y) / 2.0,
			],
		}
	}
	
	pub fn rotation_limits(self) -> (Vec3, Vec3) {
		(
			vector![self.bias.x - self.swing, self.bias.y - self.swing, self.twist.0],
			vector![self.bias.x + self.swing, self.bias.y + self.swing, self.twist.1],
		)
	}
}
