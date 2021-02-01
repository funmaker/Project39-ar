use crate::math::{Color, Point3, Vec3, Similarity3, Translation3};

#[derive(Debug, Clone)]
pub struct Bone {
	pub name: String,
	pub parent: Option<usize>,
	pub color: Color,
	pub inv_model_transform: Translation3,
	pub local_transform: Translation3,
	pub anim_transform: Similarity3,
	pub display: bool,
	pub connection: BoneConnection,
}

impl Bone {
	pub fn new(name: impl Into<String>, parent: Option<usize>, color: Color, model_pos: &Vec3, local_pos: &Vec3, display: bool, connection: BoneConnection) -> Self {
		Bone {
			name: name.into(),
			parent,
			color,
			inv_model_transform: (-model_pos).into(),
			local_transform: local_pos.clone().into(),
			anim_transform: Similarity3::identity(),
			display,
			connection,
		}
	}
	
	pub fn rest_pos(&self) -> Point3 {
		self.inv_model_transform.inverse_transform_point(&Point3::origin())
	}
}

#[derive(Debug, Clone)]
pub enum BoneConnection {
	None,
	Bone(usize),
	Offset(Vec3),
}
