use crate::math::IntoArray;

#[derive(Default, Copy, Clone)]
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
