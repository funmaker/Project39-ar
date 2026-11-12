use std::ops::Deref;
use std::sync::Arc;
use enumflags2::bitflags;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::render_pass::Framebuffer;

use crate::math::{AMat4, Isometry3, PMat4, Vec2};
use crate::utils::FramebufferBundle;


/// Rendering phase. Opaque geometry is drawn first, then transparent geometry back to front.
#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RenderType {
	Opaque,
	Transparent,
}

/// Per-frame parameters of a render target.
pub struct RenderTargetContext {
	pub framebuffer: Arc<Framebuffer>,
	/// World to eye transforms of both eyes.
	pub view: (AMat4, AMat4),
	/// Projection matrices of both eyes.
	pub projection: (PMat4, PMat4),
	/// Horizontal and vertical field of view of both eyes, in radians.
	pub fov: (Vec2, Vec2),
	pub framebuffer_size: (u32, u32),
	/// Size of a pixel in normalized device coordinates, taking SSAA into account.
	pub pixel_scale: Vec2,
	pub ssaa: f32,
}

impl RenderTargetContext {
	pub fn new(fb: FramebufferBundle, view: (AMat4, AMat4), projection: (PMat4, PMat4), fov: (Vec2, Vec2)) -> Self {
		let framebuffer_size = fb.extent();
		
		RenderTargetContext {
			framebuffer: fb.framebuffer,
			view,
			projection,
			fov,
			framebuffer_size,
			pixel_scale: vector!(1.0 / framebuffer_size.0 as f32, 1.0 / framebuffer_size.1 as f32) * fb.ssaa,
			ssaa: fb.ssaa,
		}
	}
}

/// Context passed to the render hooks of components. Dereferences to the `RenderTargetContext`.
pub struct RenderContext<'a> {
	rt_context: &'a RenderTargetContext,
	/// Phase currently being rendered.
	pub render_type: RenderType,
	/// Command buffer the frame is recorded into.
	pub builder: &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
	/// Camera (head) position of the frame.
	pub camera_pos: Isometry3,
}

impl<'a> RenderContext<'a> {
	pub fn new(rt_context: &'a RenderTargetContext, builder: &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, camera_pos: Isometry3) -> Self {
		RenderContext {
			rt_context,
			render_type: RenderType::Opaque,
			builder,
			camera_pos,
		}
	}
}

impl<'a> Deref for RenderContext<'a> {
	type Target = RenderTargetContext;
	
	fn deref(&self) -> &Self::Target {
		self.rt_context
	}
}
