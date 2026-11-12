use std::sync::Arc;
use anyhow::Result;
use vulkano::format::ClearValue;
use vulkano::image::Image;

use crate::math::Isometry3;
use super::{RenderTargetContext, Renderer, RenderContext};


/// Something the scene can be rendered into, such as the HMD eyes or the desktop window.
/// The hooks are called by `Renderer::render` in the order they are declared.
pub trait RenderTarget {
	/// Prepares the target for a frame rendered from `camera_pos`, returning its framebuffer and
	/// view/projection matrices. Returns `None` to skip rendering this frame.
	fn create_context(&mut self, camera_pos: Isometry3) -> Result<Option<RenderTargetContext>>;
	/// Clear values for the framebuffer attachments.
	fn clear_values(&self) -> &[Option<ClearValue>];
	/// The color image containing the last rendered frame.
	fn last_frame(&self) -> &Arc<Image>;
	
	/// Called after components' `before_render`, before the render pass begins.
	fn before_render(&mut self, _context: &mut RenderContext, _renderer: &mut Renderer) -> Result<()> { Ok(()) }
	/// Called at the start of the render pass, before any entity is drawn (eg. to draw a background).
	fn early_render(&mut self, _context: &mut RenderContext, _renderer: &mut Renderer) -> Result<()> { Ok(()) }
	/// Called at the end of the render pass, after everything is drawn.
	fn late_render(&mut self, _context: &mut RenderContext, _renderer: &mut Renderer) -> Result<()> { Ok(()) }
	/// Called after the render pass ends, but before the command buffer is built.
	fn after_render(&mut self, _context: &mut RenderContext, _renderer: &mut Renderer) -> Result<()> { Ok(()) }
	/// Called after the command buffer was enqueued for execution (eg. to present or submit the image).
	fn after_execute(&mut self, _renderer: &mut Renderer) -> Result<()> { Ok(()) }
}

impl<RT: RenderTarget> RenderTarget for &mut RT {
	fn create_context(&mut self, camera_pos: Isometry3) -> Result<Option<RenderTargetContext>> {
		(**self).create_context(camera_pos)
	}
	
	fn clear_values(&self) -> &[Option<ClearValue>] {
		(**self).clear_values()
	}
	
	fn last_frame(&self) -> &Arc<Image> {
		(**self).last_frame()
	}

	fn before_render(&mut self, context: &mut RenderContext, renderer: &mut Renderer) -> Result<()> {
		(**self).before_render(context, renderer)
	}

	fn early_render(&mut self, context: &mut RenderContext, renderer: &mut Renderer) -> Result<()> {
		(**self).early_render(context, renderer)
	}

	fn late_render(&mut self, context: &mut RenderContext, renderer: &mut Renderer) -> Result<()> {
		(**self).late_render(context, renderer)
	}

	fn after_render(&mut self, context: &mut RenderContext, renderer: &mut Renderer) -> Result<()> {
		(**self).after_render(context, renderer)
	}

	fn after_execute(&mut self, renderer: &mut Renderer) -> Result<()> {
		(**self).after_execute(renderer)
	}
}
