use err_derive::Error;
use mmd::pmx::bone::{Bone, Connection};
use mmd::pmx::joint::{Joint, JointType};
use mmd::pmx::rigid_body::{PhysicsMode, RigidBody, ShapeType};
use serde_derive::{Deserialize, Serialize};

use crate::math::{Vec3, PI};
use super::asset::{MMDIndexConfig, JointEx};


#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct MMDConfig {
	#[serde(skip_serializing_if = "Vec::is_empty")] #[serde(default)] pub rigid_bodies: Vec<MMDRigidBodyOverride>,
	#[serde(skip_serializing_if = "Vec::is_empty")] #[serde(default)] pub joints: Vec<MMDJointOverride>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct MMDRigidBodyOverride {
	pub id: Option<usize>,
	pub pattern: Option<String>,
	pub name: Option<String>,
	pub translation: Option<String>,
	pub bone_index: Option<i32>,
	pub group_id: Option<u8>,
	pub collision_mask: Option<u16>,
	#[serde(default)] #[serde(with = "shape_type_serde")] pub shape: Option<ShapeType>,
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
			pattern: None,
			name: Some(rb.local_name.clone()),
			translation: Some(rb.universal_name.clone()),
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
	
	pub fn apply_to(&self, rb: &mut RigidBody<MMDIndexConfig>) {
		if let Some(value) = &self.name            { rb.local_name = value.clone(); }
		if let Some(value) = &self.translation     { rb.universal_name = value.clone(); }
		if let Some(value) = self.bone_index       { rb.bone_index = value; }
		if let Some(value) = self.group_id         { rb.group_id = value; }
		if let Some(value) = self.collision_mask   { rb.non_collision_mask = value; }
		if let Some(value) = self.shape            { rb.shape = value; }
		if let Some(value) = self.size             { rb.shape_size = value; }
		if let Some(value) = self.position         { rb.shape_position = value; }
		if let Some(value) = self.rotation         { rb.shape_rotation = value; }
		if let Some(value) = self.mass             { rb.mass = value; }
		if let Some(value) = self.move_attenuation { rb.move_attenuation = value; }
		if let Some(value) = self.rotation_damping { rb.rotation_damping = value; }
		if let Some(value) = self.repulsion        { rb.repulsion = value; }
		if let Some(value) = self.fiction          { rb.fiction = value; }
	}
}

impl Into<RigidBody<MMDIndexConfig>> for MMDRigidBodyOverride {
	fn into(self) -> RigidBody<MMDIndexConfig> {
		let mut rb = RigidBody {
			local_name: "New RigidBody".into(),
			universal_name: "".into(),
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
		};
		
		self.apply_to(&mut rb);
		
		rb
	}
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyPart {
	Hip,
	Abdomen,
	Torso,
	Neck,
	Head,
	LeftThigh,
	RightThigh,
	LeftCalf,
	RightCalf,
	LeftFoot,
	RightFoot,
	LeftForearm,
	RightForearm,
	LeftArm,
	RightArm,
	LeftHand,
	RightHand,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct MMDJointOverride {
	pub id: Option<usize>,
	pub pattern: Option<String>,
	pub name: Option<String>,
	pub using_bone: Option<usize>,
	pub translation: Option<String>,
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
	pub body_part: Option<BodyPart>,
}

impl MMDJointOverride {
	pub fn from_mmd(joint: &JointEx<MMDIndexConfig>, id: usize) -> MMDJointOverride {
		MMDJointOverride {
			id: Some(id),
			pattern: None,
			using_bone: None,
			name: Some(joint.local_name.clone()),
			translation: Some(joint.universal_name.clone()),
			rigid_body_a: Some(joint.rigid_body_a),
			rigid_body_b: Some(joint.rigid_body_b),
			position: Some(joint.position),
			rotation: Some(joint.rotation),
			position_min: Some(joint.position_min),
			position_max: Some(joint.position_max),
			rotation_min: Some(joint.rotation_min / PI * 180.0),
			rotation_max: Some(joint.rotation_max / PI * 180.0),
			position_spring: Some(joint.position_spring),
			rotation_spring: Some(joint.rotation_spring),
			body_part: joint.body_part,
		}
	}
	
	pub fn apply_to(&self, joint: &mut JointEx<MMDIndexConfig>) {
		if let Some(value) = &self.name           { joint.local_name = value.clone(); }
		if let Some(value) = &self.translation    { joint.universal_name = value.clone(); }
		if let Some(value) = self.rigid_body_a    { joint.rigid_body_a = value; }
		if let Some(value) = self.rigid_body_b    { joint.rigid_body_b = value; }
		if let Some(value) = self.position        { joint.position = value; }
		if let Some(value) = self.rotation        { joint.rotation = value; }
		if let Some(value) = self.position_min    { joint.position_min = value; }
		if let Some(value) = self.position_max    { joint.position_max = value; }
		if let Some(value) = self.rotation_min    { joint.rotation_min = value * PI / 180.0; }
		if let Some(value) = self.rotation_max    { joint.rotation_max = value * PI / 180.0; }
		if let Some(value) = self.position_spring { joint.position_spring = value; }
		if let Some(value) = self.rotation_spring { joint.rotation_spring = value; }
		if let Some(value) = self.body_part       { joint.body_part = Some(value); }
	}
	
	pub fn normalize(&mut self, bones: &[Bone<MMDIndexConfig>], rigid_bodies: &[RigidBody<MMDIndexConfig>]) -> Result<(), MMDOverrideError> {
		if let Some(bone_id) = self.using_bone {
			let bone = bones.get(bone_id)
			                .ok_or(MMDOverrideError::BoneNotFound(bone_id))?;
			
			if self.position.is_none() {
				self.position = Some(bone.position);
			}
			
			if self.rotation.is_none() {
				let offset = match &bone.connection {
					Connection::Index(con_id) => {
						if *con_id < 0 { return Err(MMDOverrideError::InvalidOffset(bone_id)) }
						
						let position = bones.get(*con_id as usize)
						                    .ok_or(MMDOverrideError::BoneNotFound(*con_id as usize))?
							.position;
						
						position - bone.position
					},
					Connection::Position(offset) => {
						if offset.magnitude_squared() < f32::EPSILON { return Err(MMDOverrideError::InvalidOffset(bone_id)); }
						
						*offset
					},
				};
				
				let offset = -offset.normalize();
				let pitch = offset.y.asin();
				let yaw = if offset.x.abs() + offset.z.abs() < f32::EPSILON { PI } else { f32::atan2(-offset.x, -offset.z) };
				
				self.rotation = Some(Vec3::new(pitch, yaw, 0.0));
			}
			
			for (rb_id, rb) in rigid_bodies.iter().enumerate() {
				if rb.bone_index == bone_id as i32 {
					self.rigid_body_a = Some(rb_id as i32);
					break;
				}
			}
			if self.rigid_body_a.is_none() { return Err(MMDOverrideError::NoRigidBodies(bone_id)); }
			
			let mut parent_id = bone.parent;
			'outer:
			while parent_id >= 0 {
				for (rb_id, rb) in rigid_bodies.iter().enumerate() {
					if rb.bone_index == parent_id {
						self.rigid_body_b = Some(rb_id as i32);
						break 'outer;
					}
				}
				
				parent_id = bones[parent_id as usize].parent;
			}
			if self.rigid_body_b.is_none() { return Err(MMDOverrideError::AncestorsNoRigidBodies(bone_id)); }
		}
		
		Ok(())
	}
}

impl Into<JointEx<MMDIndexConfig>> for MMDJointOverride {
	fn into(self) -> JointEx<MMDIndexConfig> {
		let mut joint = Joint {
			local_name: "New Joint".into(),
			universal_name: "".into(),
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
		}.into();
		
		self.apply_to(&mut joint);
		
		joint
	}
}

#[derive(Debug, Error)]
pub enum MMDOverrideError {
	#[error(display = "No such bone id {}", _0)] BoneNotFound(usize),
	#[error(display = "Bone id {} has no parent", _0)] NoParent(usize),
	#[error(display = "Bone id {} has no connection, can't determine orientation.", _0)] InvalidOffset(usize),
	#[error(display = "Bone id {} has no rigid bodies", _0)] NoRigidBodies(usize),
	#[error(display = "Ancestors of bone id {} have no rigid bodies", _0)] AncestorsNoRigidBodies(usize),
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
