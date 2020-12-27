
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
