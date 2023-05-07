use rapier3d::prelude::{ColliderHandle, ImpulseJointHandle, RigidBodyHandle};

use crate::math::Isometry3;


pub struct MMDRigidBody {
	pub name: String,
	pub parent: Option<usize>,
	pub bone: usize,
	pub handle: RigidBodyHandle,
	pub colliders: Vec<ColliderHandle>,
	pub joint: ImpulseJointHandle,
	pub rest_pos: Isometry3,
	pub inv_bone_pos: Isometry3,
}

impl MMDRigidBody {
	pub fn new(name: impl Into<String>,handle: RigidBodyHandle, bone: usize, rest_pos: Isometry3, inv_bone_pos: Isometry3) -> Self {
		MMDRigidBody {
			name: name.into(),
			parent: None,
			bone,
			handle,
			colliders: Vec::new(),
			joint: ImpulseJointHandle::invalid(),
			rest_pos,
			inv_bone_pos,
		}
	}
}
