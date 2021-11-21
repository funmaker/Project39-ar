use crate::math::{Isometry3, Vec3};

pub type JointType = mmd::pmx::joint::JointType;

pub struct JointDesc {
	pub name: String,
	pub joint_type: JointType,
	pub rigid_body_a: usize,
	pub rigid_body_b: usize,
	pub position: Isometry3,
	pub position_min: Vec3,
	pub position_max: Vec3,
	pub rotation_min: Vec3,
	pub rotation_max: Vec3,
	pub position_spring: Vec3,
	pub rotation_spring: Vec3,
}

impl JointDesc {
	pub fn new(name: impl Into<String>,
	           joint_type: JointType,
	           rigid_body_a: usize,
	           rigid_body_b: usize,
	           position: Isometry3,
	           position_min: Vec3,
	           position_max: Vec3,
	           rotation_min: Vec3,
	           rotation_max: Vec3,
	           position_spring: Vec3,
	           rotation_spring: Vec3) -> Self {
		JointDesc {
			name: name.into(),
			joint_type,
			rigid_body_a,
			rigid_body_b,
			position,
			position_min,
			position_max,
			rotation_min,
			rotation_max,
			position_spring,
			rotation_spring,
		}
	}
}
