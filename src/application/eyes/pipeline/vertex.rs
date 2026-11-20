use std::convert::TryInto;
use vulkano::buffer::BufferContents;
use vulkano::pipeline::graphics::vertex_input::Vertex as VertexTy;

use crate::math::IntoArray;


#[repr(C)]
#[derive(Default, Copy, Clone, BufferContents, VertexTy)]
pub struct Vertex {
	#[format(R32G32_SFLOAT)]
	pos: [f32; 2],
}

impl Vertex {
	pub fn new(pos: impl IntoArray<[f32; 2]>) -> Self {
		Vertex {
			pos: pos.into_array(),
		}
	}
}

/// Depth mesh vertex (raw tracking space).
#[repr(C)]
#[derive(Default, Copy, Clone, BufferContents, VertexTy)]
pub struct MeshVertex {
	#[format(R32G32B32_SFLOAT)]
	pos: [f32; 3],
}

impl MeshVertex {
	pub fn new(pos: impl IntoArray<[f32; 3]>) -> Self {
		MeshVertex {
			pos: pos.into_array(),
		}
	}
}
