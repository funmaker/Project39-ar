use crate::math::{IntoArray};

#[derive(Default, Copy, Clone, Debug)]
pub struct Vertex {
	pos: [f32; 3],
	color: [f32; 4],
}

vulkano::impl_vertex!(Vertex, pos, color);

impl Vertex {
	pub fn new(pos: impl IntoArray<[f32; 3]>, color: impl IntoArray<[f32; 4]>) -> Self {
		Vertex {
			pos: pos.into_array(),
			color: color.into_array(),
		}
	}
}

#[derive(Default, Copy, Clone, Debug)]
pub struct TexturedVertex {
	pos: [f32; 3],
	uv: [f32; 2],
	color: [f32; 4],
}

vulkano::impl_vertex!(TexturedVertex, pos, uv, color);

impl TexturedVertex {
	pub fn new(pos: impl IntoArray<[f32; 3]>, uv: impl IntoArray<[f32; 2]>, color: impl IntoArray<[f32; 4]>) -> Self {
		TexturedVertex {
			pos: pos.into_array(),
			uv: uv.into_array(),
			color: color.into_array(),
		}
	}
}
