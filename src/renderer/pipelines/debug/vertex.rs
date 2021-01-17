
#[derive(Default, Copy, Clone, Debug)]
pub struct Vertex {
	pos: [f32; 3],
	color: [f32; 4],
}

vulkano::impl_vertex!(Vertex, pos, color);

impl Vertex {
	pub const fn new(pos: [f32; 3], color: [f32; 4]) -> Self {
		Vertex { pos, color }
	}
}

#[derive(Default, Copy, Clone, Debug)]
pub struct TexturedVertex {
	pos: [f32; 3],
	uv: [f32; 2],
	color: [f32; 4],
}

vulkano::impl_vertex!(TexturedVertex, pos, uv, color);

impl TexturedVertex {
	pub const fn new(pos: [f32; 3], uv: [f32; 2], color: [f32; 4]) -> Self {
		TexturedVertex { pos, uv, color }
	}
}
