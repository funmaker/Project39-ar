use bytemuck::{Pod, Zeroable};
use crate::math::IntoArray;

#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
pub struct Vertex {
	pos: [f32; 2],
}

vulkano::impl_vertex!(Vertex, pos);

impl Vertex {
	pub fn new(pos: impl IntoArray<[f32; 2]>) -> Self {
		Vertex {
			pos: pos.into_array(),
		}
	}
}
