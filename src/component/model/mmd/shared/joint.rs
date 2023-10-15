use crate::math::{Isometry3, Vec3};
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
	           body_part: Option<BodyPart>) -> Self {
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
		}
	}
}
