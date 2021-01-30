use crate::math::IntoArray;

#[derive(Default, Copy, Clone)]
pub struct Vertex {
	pos: [f32; 3],
	normal: [f32; 3],
	uv: [f32; 2],
	edge_scale: f32,
	bones_indices: [u32; 4],
	bones_weights: [f32; 4],
}

vulkano::impl_vertex!(Vertex, pos, normal, uv, edge_scale, bones_indices, bones_weights);

impl Vertex {
	pub fn new(pos: impl IntoArray<[f32; 3]>, normal: impl IntoArray<[f32; 3]>, uv: impl IntoArray<[f32; 2]>, edge_scale: f32, bones_indices: impl IntoArray<[u32; 4]>, bones_weights: impl IntoArray<[f32; 4]>) -> Self {
		Vertex {
			pos: pos.into_array(),
			normal: normal.into_array(),
			uv: uv.into_array(),
			edge_scale,
			bones_indices: bones_indices.into_array(),
			bones_weights: bones_weights.into_array(),
		}
	}
}
