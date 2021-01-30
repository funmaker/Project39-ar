use crate::math::{Color, Point3, Vec3, Similarity3};

#[derive(Debug, Clone)]
pub struct Bone {
	pub name: String,
	pub parent: Option<usize>,
	pub color: Color,
	pub transform: Similarity3,
	pub orig: Point3,
	pub display: bool,
	pub connection: BoneConnection,
}

impl Bone {
	pub fn new(name: impl Into<String>, parent: Option<usize>, color: Color, transform: Similarity3, orig: Point3, display: bool, connection: BoneConnection) -> Self {
		Bone {
			name: name.into(),
			parent,
			color,
			transform,
			orig,
			display,
			connection,
		}
	}
}

#[derive(Debug, Clone)]
pub enum BoneConnection {
	None,
	Bone(usize),
	Offset(Vec3),
}
