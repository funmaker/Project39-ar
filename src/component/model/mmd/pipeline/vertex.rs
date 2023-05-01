use std::convert::TryInto;
use vulkano::buffer::BufferContents;
use vulkano::pipeline::graphics::vertex_input::Vertex as VertexTy;
use crate::math::IntoArray;

#[repr(C)]
#[derive(Default, Copy, Clone, BufferContents, VertexTy)]
pub struct Vertex {
	#[format(R32G32B32_SFLOAT)]
	pos: [f32; 3],
	#[format(R32G32B32_SFLOAT)]
	normal: [f32; 3],
	#[format(R32G32_SFLOAT)]
	uv: [f32; 2],
	#[format(R32_SFLOAT)]
	edge_scale: f32,
	#[format(R32G32B32A32_UINT)]
	bones_indices: [u32; 4],
	#[format(R32G32B32A32_SFLOAT)]
	bones_weights: [f32; 4],
	#[format(R32G32B32_SFLOAT)]
	sdef_c: [f32; 3],
	#[format(R32G32B32_SFLOAT)]
	sdef_r0: [f32; 3],
	#[format(R32G32B32_SFLOAT)]
	sdef_r1: [f32; 3],
}

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
