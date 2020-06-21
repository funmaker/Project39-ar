use obj::TexturedVertex;
use openvr::render_models;

#[derive(Default, Copy, Clone)]
pub struct Vertex {
	pos: [f32; 3],
	uv: [f32; 2],
}

vulkano::impl_vertex!(Vertex, pos, uv);

impl Vertex {
	pub const fn new(x: f32, y: f32, z: f32, u: f32, v: f32) -> Self {
		Vertex {
			pos: [x, y, z],
			uv: [u, v],
		}
	}
}

impl From<&TexturedVertex> for Vertex {
	fn from(vertex: &TexturedVertex) -> Self {
		Vertex::new(
			vertex.position[0],
			vertex.position[1],
			vertex.position[2],
			vertex.texture[0],
			1.0 - vertex.texture[1],
		)
	}
}

impl From<&render_models::Vertex> for Vertex {
	fn from(vertex: &render_models::Vertex) -> Self {
		Vertex::new(
			vertex.position[0],
			vertex.position[1],
			vertex.position[2],
			vertex.texture_coord[0],
			vertex.texture_coord[1],
		)
	}
}
