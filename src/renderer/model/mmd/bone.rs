use std::cell::RefCell;
use std::rc::Rc;
use cgmath::{Matrix4, Vector3, SquareMatrix, Vector4, Zero};

use crate::debug;

pub type BoneRef = Rc<RefCell<Bone>>;

#[derive(Debug, Clone)]
pub struct Bone {
	pub id: usize,
	pub name: String,
	pub color: Vector4<f32>,
	pub transform: Matrix4<f32>,
	pub orig: Vector3<f32>,
	pub display: bool,
	pub connection: BoneConnection,
	pub children: Vec<BoneRef>,
}

impl Bone {
	pub fn new(id: usize, name: impl Into<String>) -> Self {
		Bone {
			id,
			name: name.into(),
			color: Vector4::zero(),
			transform: Matrix4::identity(),
			orig: Vector3::new(0.0, 0.0, 0.0),
			display: false,
			connection: BoneConnection::None,
			children: vec![],
		}
	}
	
	pub fn len(&self) -> usize {
		self.children.iter().map(|bone| bone.borrow().len()).fold(1, |acc, val| acc + val)
	}
	
	pub fn debug_draw(&self, model_matrix: Matrix4<f32>) {
		if self.display {
			let col = self.color;
			
			let pos = (model_matrix * self.orig.extend(1.0)).truncate();
			debug::draw_point(pos, 10.0, col);
			debug::draw_text(&self.name, pos, debug::DebugOffset::bottom_right(8.0, 8.0), 32.0, col);
			
			match &self.connection {
				BoneConnection::None => {}
				BoneConnection::Bone(con) => {
					let cpos = (model_matrix * con.borrow().orig.extend(1.0)).truncate();
					debug::draw_line(pos, cpos, 3.0, col);
				}
				BoneConnection::Offset(_cpos) => {
					// let cpos = (model_matrix * (Vector3::from(bone.position) + Vector3::from(cpos)).extend(1.0)).truncate();
					// debug::draw_line(pos, cpos, 3.0, col);
				}
			}
		}
		
		self.children.iter().for_each(|bone| bone.borrow().debug_draw(model_matrix));
	}
}

#[derive(Debug, Clone)]
pub enum BoneConnection {
	None,
	Bone(BoneRef),
	Offset(Vector3<f32>),
}

#[derive(Debug, Clone)]
pub struct BoneUBO {
	transform: Matrix4<f32>,
	orig: Vector4<f32>,
}

impl BoneUBO {
	pub fn new() -> Self {
		BoneUBO {
			transform: Matrix4::identity(),
			orig: Vector4::zero(),
		}
	}
}
