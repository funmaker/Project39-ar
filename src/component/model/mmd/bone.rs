use crate::application::EntityRef;
use crate::math::{Color, Point3, Vec3, Similarity3, Translation3, Isometry3};


#[derive(Debug, Clone)]
pub struct MMDBone {
	pub name: String,
	pub parent: Option<usize>,
	pub color: Color,
	pub inv_model_transform: Translation3,
	pub local_transform: Translation3,
	pub anim_transform: Similarity3,
	pub transform_override: Option<Similarity3>,
	pub display: bool,
	pub connection: BoneConnection,
	pub rigid_body: EntityRef,
	pub inv_rigid_body_transform: Isometry3,
}

impl MMDBone {
	pub fn new(name: impl Into<String>, parent: Option<usize>, color: Color, model_pos: Vec3, local_pos: Vec3, display: bool, connection: BoneConnection) -> Self {
		MMDBone {
			name: name.into(),
			parent,
			color,
			inv_model_transform: (-model_pos).into(),
			local_transform: local_pos.into(),
			anim_transform: Similarity3::identity(),
			transform_override: None,
			display,
			connection,
			rigid_body: EntityRef::null(),
			inv_rigid_body_transform: Isometry3::identity(),
		}
	}
	
	pub fn origin(&self) -> Point3 {
		self.inv_model_transform.inverse_transform_point(&Point3::origin())
	}
	
	pub fn attach_rigid_body(&mut self, rigid_body: EntityRef, model_pos: Isometry3) -> &mut Self {
		self.rigid_body = rigid_body;
		self.inv_rigid_body_transform = (self.inv_model_transform * model_pos).inverse();
		self
	}
}

#[derive(Debug, Clone)]
pub enum BoneConnection {
	None,
	Bone(usize),
	Offset(Vec3),
}
