use crate::math::IntoArray;

#[derive(Default, Copy, Clone)]
pub struct Vertex {
	pos: [f32; 3],
	normal: [f32; 3],
	uv: [f32; 2],
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
