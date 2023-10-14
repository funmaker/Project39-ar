use crate::application::EntityRef;
use crate::math::{Color, Point3, Similarity3, Translation3, Isometry3};
use super::shared::{BoneDesc, BoneConnection};


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
	pub fn origin(&self) -> Point3 {
		self.inv_model_transform.inverse_transform_point(&Point3::origin())
	}
	
	pub fn attach_rigid_body(&mut self, rigid_body: EntityRef, model_pos: Isometry3) -> &mut Self {
		self.rigid_body = rigid_body;
		self.inv_rigid_body_transform = (self.inv_model_transform * model_pos).inverse();
		self
	}
}

impl From<&BoneDesc> for MMDBone {
	fn from(desc: &BoneDesc) -> Self {
		MMDBone {
			name: desc.name.clone(),
			parent: desc.parent,
			color: desc.color,
			inv_model_transform: (-desc.model_pos).into(),
			local_transform: desc.local_pos.into(),
			anim_transform: Similarity3::identity(),
			transform_override: None,
			display: desc.display,
			connection: desc.connection,
			rigid_body: EntityRef::null(),
			inv_rigid_body_transform: Isometry3::identity(),
		}
	}
}
