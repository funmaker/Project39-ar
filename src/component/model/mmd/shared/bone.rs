use crate::math::{Color, Vec3};


/// Bone of an MMD model as loaded from the file.
pub struct BoneDesc {
	pub id: usize,
	pub name: String,
	pub parent: Option<usize>,
	pub color: Color,
	pub display: bool,
	pub connection: BoneConnection,
	/// Rest position in model space.
	pub model_pos: Vec3,
	/// Rest position relative to the parent bone.
	pub local_pos: Vec3,
}

/// What a bone points to: nothing, another bone, or a fixed offset.
#[derive(Debug, Clone, Copy)]
pub enum BoneConnection {
	None,
	Bone(usize),
	Offset(Vec3),
}

impl BoneDesc {
	pub fn new(name: impl Into<String>, parent: Option<usize>, color: Color, model_pos: Vec3, local_pos: Vec3, display: bool, connection: BoneConnection) -> Self {
		BoneDesc {
			id: 0,
			name: name.into(),
			parent,
			color,
			model_pos,
			local_pos,
			display,
			connection,
		}
	}
	
	/// Sets the bone id.
	pub fn id(self, id: usize) -> Self {
		BoneDesc {
			id,
			..self
		}
	}
}
