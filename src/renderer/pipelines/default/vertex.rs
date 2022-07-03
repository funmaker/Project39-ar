use bytemuck::{Pod, Zeroable};
use crate::math::IntoArray;

#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
pub struct Vertex {
	pub pos: [f32; 3],
	pub normal: [f32; 3],
	pub uv: [f32; 2],
}

vulkano::impl_vertex!(Vertex, pos, normal, uv);

impl Vertex {
	pub fn new(pos: impl IntoArray<[f32; 3]>, normal: impl IntoArray<[f32; 3]>, uv: impl IntoArray<[f32; 2]>) -> Self {
		Vertex {
			pos: pos.into_array(),
			normal: normal.into_array(),
			uv: uv.into_array(),
		}
	}
}
