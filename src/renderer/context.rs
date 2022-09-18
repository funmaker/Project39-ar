use std::ops::Deref;
use std::sync::Arc;
use enumflags2::bitflags;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::render_pass::Framebuffer;

use crate::math::{AMat4, Isometry3, PMat4, Vec2};
use crate::utils::FramebufferBundle;

#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RenderType {
	Opaque,
	Transparent,
}

pub struct RenderTargetContext {
	pub framebuffer: Arc<Framebuffer>,
	pub view: (AMat4, AMat4),
	pub projection: (PMat4, PMat4),
	pub fov: (Vec2, Vec2),
	pub framebuffer_size: (u32, u32),
	pub pixel_scale: Vec2,
	pub ssaa: f32,
}

impl RenderTargetContext {
	pub fn new(fb: FramebufferBundle, view: (AMat4, AMat4), projection: (PMat4, PMat4), fov: (Vec2, Vec2)) -> Self {
		let framebuffer_size = fb.size();
		
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

pub struct RenderContext<'a> {
	rt_context: &'a RenderTargetContext,
	pub render_type: RenderType,
	pub builder: &'a mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
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
