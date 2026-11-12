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

/// Creates all available tools in the menu order. New tools must be added here.
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

/// A tool usable with the `ToolGun`.
#[allow(unused_variables)]
pub trait Tool {
	/// Name shown in the menu and on the gun's screen.
	fn name(&self) -> &str;
	// fn activate(&mut self, toolgun: &ToolGun, application: &Application) -> Result<()> { Ok(()) }
	/// Called every frame while the tool is selected and the gun is held by `hand`.
	fn tick(&mut self, toolgun: &ToolGun, hand: Hand, ray: Ray, application: &Application) -> Result<()> { Ok(()) }
	/// Called before rendering while the tool is active.
	fn pre_render(&mut self, toolgun: &ToolGun, context: &mut RenderContext) -> Result<()> { Ok(()) }
	/// Renders tool specific visuals while the tool is active.
	fn render(&mut self, toolgun: &ToolGun, context: &mut RenderContext) -> Result<()> { Ok(()) }
	// fn deactivate(&mut self, toolgun: &ToolGun, application: &Application) -> Result<()> { Ok(()) }
}
