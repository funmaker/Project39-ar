use cgmath::{Matrix4, Vector3, Vector4};

#[derive(Debug, Clone)]
pub struct Bone {
	pub name: String,
	pub parent: Option<usize>,
	pub color: Vector4<f32>,
	pub transform: Matrix4<f32>,
	pub orig: Vector3<f32>,
	pub display: bool,
	pub connection: BoneConnection,
}

impl Bone {
	pub fn new(name: impl Into<String>, parent: Option<usize>, color: Vector4<f32>, transform: Matrix4<f32>, orig: Vector3<f32>, display: bool, connection: BoneConnection) -> Self {
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
	Offset(Vector3<f32>),
}
