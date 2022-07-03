use bytemuck::{Pod, Zeroable};
use crate::math::{IntoArray};

#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
pub struct Vertex {
	pos_left: [f32; 3],
	pos_right: [f32; 3],
	color: [f32; 4],
}

vulkano::impl_vertex!(Vertex, pos_left, pos_right, color);

impl Vertex {
	pub fn new(pos_left: impl IntoArray<[f32; 3]>, pos_right: impl IntoArray<[f32; 3]>, color: impl IntoArray<[f32; 4]>) -> Self {
		Vertex {
			pos_left: pos_left.into_array(),
			pos_right: pos_right.into_array(),
			color: color.into_array(),
		}
	}
}

#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
pub struct TexturedVertex {
	pos_left: [f32; 3],
	pos_right: [f32; 3],
	uv: [f32; 2],
	color: [f32; 4],
}

vulkano::impl_vertex!(TexturedVertex, pos_left, pos_right, uv, color);

impl TexturedVertex {
	pub fn new(pos_left: impl IntoArray<[f32; 3]>, pos_right: impl IntoArray<[f32; 3]>, uv: impl IntoArray<[f32; 2]>, color: impl IntoArray<[f32; 4]>) -> Self {
		TexturedVertex {
			pos_left: pos_left.into_array(),
			pos_right: pos_right.into_array(),
			uv: uv.into_array(),
			color: color.into_array(),
		}
	}
}
