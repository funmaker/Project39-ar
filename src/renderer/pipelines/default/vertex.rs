
#[derive(Default, Copy, Clone)]
pub struct Vertex {
	pos: [f32; 3],
	normal: [f32; 3],
	uv: [f32; 2],
}

vulkano::impl_vertex!(Vertex, pos, normal, uv);

impl Vertex {
	pub const fn new(pos: [f32; 3], normal: [f32; 3], uv: [f32; 2]) -> Self {
		Vertex { pos, normal, uv }
	}
}
