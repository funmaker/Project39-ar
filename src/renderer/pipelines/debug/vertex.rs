use std::convert::TryInto;
use vulkano::buffer::BufferContents;
use vulkano::pipeline::graphics::vertex_input::Vertex as VertexTy;

use crate::math::IntoArray;


#[repr(C)]
#[derive(Default, Copy, Clone, BufferContents, VertexTy)]
pub struct Vertex {
	#[format(R32G32B32_SFLOAT)]
	pos_left: [f32; 3],
	#[format(R32G32B32_SFLOAT)]
	pos_right: [f32; 3],
	#[format(R32G32B32A32_SFLOAT)]
	color: [f32; 4],
}

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
#[derive(Default, Copy, Clone, BufferContents, VertexTy)]
pub struct TexturedVertex {
	#[format(R32G32B32_SFLOAT)]
	pos_left: [f32; 3],
	#[format(R32G32B32_SFLOAT)]
	pos_right: [f32; 3],
	#[format(R32G32_SFLOAT)]
	uv: [f32; 2],
	#[format(R32G32B32A32_SFLOAT)]
	color: [f32; 4],
}

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
