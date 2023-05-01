use std::convert::TryInto;
use vulkano::buffer::BufferContents;
use vulkano::pipeline::graphics::vertex_input::Vertex as VertexTy;
use crate::math::IntoArray;

#[repr(C)]
#[derive(Default, Copy, Clone, BufferContents, VertexTy)]
pub struct Vertex {
	#[format(R32G32B32_SFLOAT)]
	pub pos: [f32; 3],
	#[format(R32G32B32_SFLOAT)]
	pub normal: [f32; 3],
	#[format(R32G32_SFLOAT)]
	pub uv: [f32; 2],
}

impl Vertex {
	pub fn new(pos: impl IntoArray<[f32; 3]>, normal: impl IntoArray<[f32; 3]>, uv: impl IntoArray<[f32; 2]>) -> Self {
		Vertex {
			pos: pos.into_array(),
			normal: normal.into_array(),
			uv: uv.into_array(),
		}
	}
}
