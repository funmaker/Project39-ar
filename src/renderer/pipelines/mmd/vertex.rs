
#[derive(Default, Copy, Clone)]
pub struct Vertex {
	pos: [f32; 3],
	normal: [f32; 3],
	uv: [f32; 2],
	edge_scale: f32,
}

vulkano::impl_vertex!(Vertex, pos, normal, uv, edge_scale);

impl Vertex {
	pub const fn new(pos: [f32; 3], normal: [f32; 3], uv: [f32; 2], edge_scale: f32) -> Self {
		Vertex { pos, normal, uv, edge_scale }
	}
}
