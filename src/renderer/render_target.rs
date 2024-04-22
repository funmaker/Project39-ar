use std::sync::Arc;
use anyhow::Result;
use vulkano::format::ClearValue;
use vulkano::image::AttachmentImage;

use crate::math::Isometry3;
use super::{RenderTargetContext, Renderer, RenderContext};


pub trait RenderTarget {
	fn create_context(&mut self, camera_pos: Isometry3) -> Result<Option<RenderTargetContext>>;
	fn clear_values(&self) -> &[Option<ClearValue>];
	fn last_frame(&self) -> &Arc<AttachmentImage>;
	
	fn before_render(&mut self, _context: &mut RenderContext, _renderer: &mut Renderer) -> Result<()> { Ok(()) }
	fn early_render(&mut self, _context: &mut RenderContext, _renderer: &mut Renderer) -> Result<()> { Ok(()) }
	fn late_render(&mut self, _context: &mut RenderContext, _renderer: &mut Renderer) -> Result<()> { Ok(()) }
	fn after_render(&mut self, _context: &mut RenderContext, _renderer: &mut Renderer) -> Result<()> { Ok(()) }
	fn after_execute(&mut self, _renderer: &mut Renderer) -> Result<()> { Ok(()) }
}
