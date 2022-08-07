use mmd::pmx::joint::{Joint, JointType};
use mmd::pmx::rigid_body::{PhysicsMode, RigidBody, ShapeType};
use regex::Regex;
use serde_derive::{Deserialize, Serialize};

use crate::math::Vec3;
use super::asset::MMDIndexConfig;

#[derive(Serialize, Deserialize, Default)]
pub struct MMDConfig {
	#[serde(default)] pub rigid_bodies: Vec<MMDRigidBodyOverride>,
	#[serde(default)] pub joints: Vec<MMDJointOverride>,
}

#[derive(Serialize, Deserialize)]
pub struct MMDRigidBodyOverride {
	pub id: Option<usize>,
	#[serde(with = "regex_serde")] pub regex: Option<Regex>,
	pub name: Option<String>,
	pub bone_index: Option<i32>,
	pub group_id: Option<u8>,
	pub collision_mask: Option<u16>,
	#[serde(with = "shape_type_serde")] pub shape: Option<ShapeType>,
	pub size: Option<Vec3>,
	pub position: Option<Vec3>,
	pub rotation: Option<Vec3>,
	pub mass: Option<f32>,
	pub move_attenuation: Option<f32>,
	pub rotation_damping: Option<f32>,
	pub repulsion: Option<f32>,
	pub fiction: Option<f32>,
	// pub physics_mode: Option<PhysicsMode>,
}

impl MMDRigidBodyOverride {
	pub fn from_mmd(rb: &RigidBody<MMDIndexConfig>, id: usize) -> MMDRigidBodyOverride {
		MMDRigidBodyOverride {
			id: Some(id),
			regex: None,
			name: Some(rb.local_name.clone()),
			bone_index: Some(rb.bone_index),
			group_id: Some(rb.group_id),
			collision_mask: Some(rb.non_collision_mask),
			shape: Some(rb.shape),
			size: Some(rb.shape_size),
			position: Some(rb.shape_position),
			rotation: Some(rb.shape_rotation),
			mass: Some(rb.mass),
			move_attenuation: Some(rb.move_attenuation),
			rotation_damping: Some(rb.rotation_damping),
			repulsion: Some(rb.repulsion),
			fiction: Some(rb.fiction),
		}
	}
	
	// I am a code artisan
	pub fn apply_to(&self, rb: RigidBody<MMDIndexConfig>) -> RigidBody<MMDIndexConfig> {
		RigidBody {
			    universal_name: self.name     .clone().unwrap_or(rb.universal_name    ),
			        bone_index: self.bone_index       .unwrap_or(rb.bone_index        ),
			          group_id: self.group_id         .unwrap_or(rb.group_id          ),
			non_collision_mask: self.collision_mask   .unwrap_or(rb.non_collision_mask),
			             shape: self.shape            .unwrap_or(rb.shape             ),
			        shape_size: self.size             .unwrap_or(rb.shape_size        ),
			    shape_position: self.position         .unwrap_or(rb.shape_position    ),
			    shape_rotation: self.rotation         .unwrap_or(rb.shape_rotation    ),
			              mass: self.mass             .unwrap_or(rb.mass              ),
			  move_attenuation: self.move_attenuation .unwrap_or(rb.move_attenuation  ),
			  rotation_damping: self.rotation_damping .unwrap_or(rb.rotation_damping  ),
			         repulsion: self.repulsion        .unwrap_or(rb.repulsion         ),
			           fiction: self.fiction          .unwrap_or(rb.fiction           ),
			..rb
		}
	}
}

impl Into<RigidBody<MMDIndexConfig>> for MMDRigidBodyOverride {
	fn into(self) -> RigidBody<MMDIndexConfig> {
		self.apply_to(RigidBody {
			local_name: "New RigidBody".into(),
			universal_name: "New RigidBody".into(),
			bone_index: 0,
			group_id: 0,
			non_collision_mask: 0xFFFF,
			shape: ShapeType::Box,
			shape_size: vector!(1.0, 1.0, 1.0),
			shape_position: vector!(0.0, 0.0, 0.0),
			shape_rotation: vector!(0.0, 0.0, 0.0),
			mass: 1.0,
			move_attenuation: 0.999,
			rotation_damping: 0.999,
			repulsion: 0.0,
			fiction: 0.5,
			physics_mode: PhysicsMode::Dynamic,
		})
	}
}

#[derive(Serialize, Deserialize)]
pub struct MMDJointOverride {
	pub id: Option<usize>,
	#[serde(with = "regex_serde")] pub regex: Option<Regex>,
	pub name: Option<String>,
	// pub joint_type: JointType,
	pub rigid_body_a: Option<i32>,
	pub rigid_body_b: Option<i32>,
	pub position: Option<Vec3>,
	pub rotation: Option<Vec3>,
	pub position_min: Option<Vec3>,
	pub position_max: Option<Vec3>,
	pub rotation_min: Option<Vec3>,
	pub rotation_max: Option<Vec3>,
	pub position_spring: Option<Vec3>,
	pub rotation_spring: Option<Vec3>,
}

impl MMDJointOverride {
	pub fn from_mmd(rb: &Joint<MMDIndexConfig>, id: usize) -> MMDJointOverride {
		MMDJointOverride {
			id: Some(id),
			regex: None,
			name: Some(rb.local_name.clone()),
			rigid_body_a: Some(rb.rigid_body_a),
			rigid_body_b: Some(rb.rigid_body_b),
			position: Some(rb.position),
			rotation: Some(rb.rotation),
			position_min: Some(rb.position_min),
			position_max: Some(rb.position_max),
			rotation_min: Some(rb.rotation_min),
			rotation_max: Some(rb.rotation_max),
			position_spring: Some(rb.position_spring),
			rotation_spring: Some(rb.rotation_spring),
		}
	}
	
	pub fn apply_to(&self, joint: Joint<MMDIndexConfig>) -> Joint<MMDIndexConfig> {
		Joint {
			 universal_name: self.name    .clone().unwrap_or(joint.universal_name ),
			   rigid_body_a: self.rigid_body_a    .unwrap_or(joint.rigid_body_a   ),
			   rigid_body_b: self.rigid_body_b    .unwrap_or(joint.rigid_body_b   ),
			       position: self.position        .unwrap_or(joint.position       ),
			       rotation: self.rotation        .unwrap_or(joint.rotation       ),
			   position_min: self.position_min    .unwrap_or(joint.position_min   ),
			   position_max: self.position_max    .unwrap_or(joint.position_max   ),
			   rotation_min: self.rotation_min    .unwrap_or(joint.rotation_min   ),
			   rotation_max: self.rotation_max    .unwrap_or(joint.rotation_max   ),
			position_spring: self.position_spring .unwrap_or(joint.position_spring),
			rotation_spring: self.rotation_spring .unwrap_or(joint.rotation_spring),
			..joint
		}
	}
}

impl Into<Joint<MMDIndexConfig>> for MMDJointOverride {
	fn into(self) -> Joint<MMDIndexConfig> {
		self.apply_to(Joint {
			local_name: "New Joint".into(),
			universal_name: "New Joint".into(),
			joint_type: JointType::SpringFree,
			rigid_body_a: 0,
			rigid_body_b: 0,
			position: vector!(0.0, 0.0, 0.0),
			rotation: vector!(0.0, 0.0, 0.0),
			position_min: vector!(0.0, 0.0, 0.0),
			position_max: vector!(0.0, 0.0, 0.0),
			rotation_min: vector!(0.0, 0.0, 0.0),
			rotation_max: vector!(0.0, 0.0, 0.0),
			position_spring: vector!(0.0, 0.0, 0.0),
			rotation_spring: vector!(0.0, 0.0, 0.0),
		})
	}
}

mod regex_serde {
	use std::borrow::Cow;
	use regex::Regex;
	use serde::{Serializer, Serialize, Deserializer, Deserialize};
	use serde::de::Error;
	
	pub fn serialize<S: Serializer>(value: &Option<Regex>, ser: S) -> Result<S::Ok, S::Error> {
		match value {
			Some(ref value) => value.as_str().serialize(ser),
			None => ser.serialize_none(),
		}
	}
	
	pub fn deserialize<'d, D: Deserializer<'d>>(de: D) -> Result<Option<Regex>, D::Error> {
		let pat = <Option<Cow<str>>>::deserialize(de)?;
		
		match pat.as_ref().map(|pat| pat.parse::<Regex>()) {
			None => Ok(None),
			Some(Ok(regex)) => Ok(Some(regex)),
			Some(Err(err)) => Err(D::Error::custom(format!("valid regex \"{}\", {}", pat.unwrap(), err))),
		}
	}
}

mod shape_type_serde {
	use std::borrow::Cow;
	use mmd::pmx::rigid_body::ShapeType;
	use serde::{Serializer, Deserializer, Deserialize};
	use serde::de::{Error, Unexpected};
	
	pub fn serialize<S: Serializer>(value: &Option<ShapeType>, ser: S) -> Result<S::Ok, S::Error> {
		match value {
			Some(ShapeType::Sphere) => ser.serialize_str("sphere"),
			Some(ShapeType::Box) => ser.serialize_str("box"),
			Some(ShapeType::Capsule) => ser.serialize_str("capsule"),
			None => ser.serialize_none()
		}
	}
	
	pub fn deserialize<'d, D: Deserializer<'d>>(de: D) -> Result<Option<ShapeType>, D::Error> {
		let pat = <Option<Cow<str>>>::deserialize(de)?;
		
		match pat.as_ref().map(Cow::as_ref) {
			None => Ok(None),
			Some("sphere") => Ok(Some(ShapeType::Sphere)),
			Some("box") => Ok(Some(ShapeType::Box)),
			Some("capsule") => Ok(Some(ShapeType::Capsule)),
			Some(invalid) => Err(D::Error::invalid_value(Unexpected::Str(invalid), &"sphere, box or capsule")),
		}
	}
}
