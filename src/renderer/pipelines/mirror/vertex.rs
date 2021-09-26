use crate::math::IntoArray;

#[derive(Default, Copy, Clone)]
pub struct Vertex {
	pos: [f32; 3],
	uv_left: [f32; 2],
	uv_right: [f32; 2],
}

vulkano::impl_vertex!(Vertex, pos, uv_left, uv_right);

impl Vertex {
	pub fn new(pos: impl IntoArray<[f32; 3]>, uv_left: impl IntoArray<[f32; 2]>, uv_right: impl IntoArray<[f32; 2]>) -> Self {
		Vertex {
			pos: pos.into_array(),
			uv_left: uv_left.into_array(),
			uv_right: uv_right.into_array(),
		}
	}
}
