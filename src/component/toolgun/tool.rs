use anyhow::Result;

use crate::application::{Application, Hand};
use crate::math::Ray;
use crate::renderer::{RenderContext, Renderer};
use super::ToolGun;
use super::axis::Axis;
use super::remover::Remover;
use super::rope::RopeTool;
use super::spawner::Spawner;
use super::thruster::ThrusterTool;
use super::weld::Weld;


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
	// fn activate(&mut self, toolgun: &ToolGun, application: &Application) -> Result<()> { Ok(()) }
	fn tick(&mut self, toolgun: &ToolGun, hand: Hand, ray: Ray, application: &Application) -> Result<()> { Ok(()) }
	fn pre_render(&mut self, toolgun: &ToolGun, context: &mut RenderContext) -> Result<()> { Ok(()) }
	fn render(&mut self, toolgun: &ToolGun, context: &mut RenderContext) -> Result<()> { Ok(()) }
	// fn deactivate(&mut self, toolgun: &ToolGun, application: &Application) -> Result<()> { Ok(()) }
}
