use vulkano::command_buffer::AutoCommandBufferBuilder;

use super::RenderError;

pub struct DebugRenderer {

}

impl DebugRenderer {
	pub fn new() -> DebugRenderer {
		DebugRenderer {
		
		}
	}
	
	pub fn render(&mut self, builder: &mut AutoCommandBufferBuilder, eye: u32) -> Result<(), RenderError> {
		Ok(())
	}
}
