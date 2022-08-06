use bytemuck::{Pod, Zeroable};
use crate::math::IntoArray;

#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
pub struct Vertex {
	pos: [f32; 3],
	normal: [f32; 3],
	uv: [f32; 2],
	edge_scale: f32,
	bones_indices: [u32; 4],
	bones_weights: [f32; 4],
	sdef_c: [f32; 3],
	sdef_r0: [f32; 3],
	sdef_r1: [f32; 3],
}

vulkano::impl_vertex!(Vertex, pos, normal, uv, edge_scale, bones_indices, bones_weights, sdef_c, sdef_r0, sdef_r1);

const SDEF_DEF: [[f32; 3]; 3] = [
	[0.0, 0.0, 0.0],
	[0.0, 0.0, 0.0],
	[0.0, 0.0, 0.0],
];

impl Vertex {
	pub fn new(
		pos: impl IntoArray<[f32; 3]>,
		normal: impl IntoArray<[f32; 3]>,
		uv: impl IntoArray<[f32; 2]>,
		edge_scale: f32,
		bones_indices: impl IntoArray<[u32; 4]>,
		bones_weights: impl IntoArray<[f32; 4]>,
		sdef: Option<[impl IntoArray<[f32; 3]>; 3]>,
	) -> Self {
		let sdef = sdef.map(|[c, r0, r1]| [
			               c.into_array(),
			               r0.into_array(),
			               r1.into_array()
		               ])
		               .unwrap_or(SDEF_DEF);
		
		Vertex {
			pos: pos.into_array(),
			normal: normal.into_array(),
			uv: uv.into_array(),
			edge_scale,
			bones_indices: bones_indices.into_array(),
			bones_weights: bones_weights.into_array(),
			sdef_c: sdef[0],
			sdef_r0: sdef[1],
			sdef_r1: sdef[2],
		}
	}
}
