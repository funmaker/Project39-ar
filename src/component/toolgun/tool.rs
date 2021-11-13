use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};

use crate::application::{Application, Hand};
use crate::math::Ray;
use crate::component::toolgun::rope::RopeTool;
use crate::renderer::Renderer;
use super::ToolGun;
use super::weld::Weld;
use super::thruster::ThrusterTool;
use super::axis::Axis;
use super::remover::Remover;
use super::spawner::Spawner;

pub type ToolError = Box<dyn std::error::Error>;

pub fn get_all_tools(renderer: &mut Renderer) -> Vec<Box<dyn Tool>> {
	vec![
		Box::new(Spawner::new()),
		Box::new(Remover::new()),
		Box::new(Axis::new()),
		Box::new(ThrusterTool::new(renderer)),
		Box::new(Weld::new()),
		Box::new(RopeTool::new()),
	]
}

#[allow(unused_variables)]
pub trait Tool {
	fn name(&self) -> &str;
	// fn activate(&mut self, toolgun: &ToolGun, application: &Application) -> Result<(), ToolError> { Ok(()) }
	fn tick(&mut self, toolgun: &ToolGun, hand: Hand, ray: Ray, application: &Application) -> Result<(), ToolError> { Ok(()) }
	fn pre_render(&mut self, toolgun: &ToolGun, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ToolError> { Ok(()) }
	fn render(&mut self, toolgun: &ToolGun, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ToolError> { Ok(()) }
	// fn deactivate(&mut self, toolgun: &ToolGun, application: &Application) -> Result<(), ToolError> { Ok(()) }
}
