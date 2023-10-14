use crate::math::{Color, Vec3};

pub struct BoneDesc {
	pub name: String,
	pub parent: Option<usize>,
	pub color: Color,
	pub display: bool,
	pub connection: BoneConnection,
	pub model_pos: Vec3,
	pub local_pos: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub enum BoneConnection {
	None,
	Bone(usize),
	Offset(Vec3),
}

impl BoneDesc {
	pub fn new(name: impl Into<String>, parent: Option<usize>, color: Color, model_pos: Vec3, local_pos: Vec3, display: bool, connection: BoneConnection) -> Self {
		BoneDesc {
			name: name.into(),
			parent,
			color,
			model_pos,
			local_pos,
			display,
			connection,
		}
	}
}
