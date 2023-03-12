use std::sync::Arc;
use std::convert::TryFrom;
use winit::window::Window as WinitWindow;
use err_derive::Error;
use egui::{Context, FullOutput, TextStyle};
use egui_vulkano::{Painter, UpdateTexturesResult};
use egui_winit::State as EguiWinit;
use vulkano::{command_buffer, render_pass};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, RenderPassBeginInfo, SubpassContents};
use vulkano::format::ClearValue;
use vulkano::image::ImageAccess;
use vulkano::image::view::ImageView;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass};
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;

use crate::renderer::{IMAGE_FORMAT, Renderer};
use crate::utils::FramebufferBundle;

pub struct WindowGui {
	ctx: Context,
	output: FullOutput,
	painter: Painter,
	winit: EguiWinit,
	render_pass: Arc<RenderPass>,
	framebuffer: Arc<Framebuffer>,
}

impl WindowGui {
	pub fn new(fb: &FramebufferBundle, event_loop: &EventLoop<()>, renderer: &Renderer) -> Result<Self, WindowGuiError> {
		let ctx = Context::default();
		
		{
			let mut style = (*ctx.style()).clone();
			let font = style.text_styles.get_mut(&TextStyle::Monospace).unwrap();
			font.size = 16.0;
			ctx.set_style(style);
		}
		
		let render_pass = vulkano::ordered_passes_renderpass!(
			renderer.device.clone(),
			attachments: {
				color: {
					load: Load,
					store: Store,
					format: IMAGE_FORMAT,
					samples: 1,
				}
			},
			passes: [
				{ color: [color], depth_stencil: {}, input: [] },
                { color: [color], depth_stencil: {}, input: [] } // Create a second renderpass to draw egui
			]
		)?;
		
		let painter = egui_vulkano::Painter::new(
			renderer.device.clone(),
			renderer.memory_allocator.clone(),
			renderer.descriptor_set_allocator.clone(),
			renderer.queue.clone(),
			Subpass::from(render_pass.clone(), 1).unwrap(),
		)?;
		
		let framebuffer = Framebuffer::new(render_pass.clone(), FramebufferCreateInfo {
			attachments: vec![ImageView::new_default(fb.main_image.clone())?],
			extent: fb.main_image.dimensions().width_height(),
			layers: 1,
			..FramebufferCreateInfo::default()
		})?;
		
		let winit = egui_winit::State::new(&**event_loop);
		
		Ok(WindowGui {
			ctx,
			output: FullOutput::default(),
			painter,
			winit,
			render_pass,
			framebuffer,
		})
	}
	
	pub fn on_event(&mut self, event: &WindowEvent) -> bool {
		self.winit.on_event(&self.ctx, &event).consumed
	}
	
	pub fn regen_framebuffer(&mut self, fb: &FramebufferBundle) -> Result<(), WindowGuiRegenFramebufferError> {
		self.framebuffer = Framebuffer::new(self.render_pass.clone(), FramebufferCreateInfo {
			attachments: vec![ImageView::new_default(fb.main_image.clone())?],
			extent: fb.main_image.dimensions().width_height(),
			layers: 1,
			..FramebufferCreateInfo::default()
		})?;
		
		Ok(())
	}
	
	pub fn is_dragging(&self) -> bool {
		self.ctx.memory().is_anything_being_dragged()
	}
	
	pub fn start_frame(&mut self, window: &WinitWindow) {
		self.ctx.begin_frame(self.winit.take_egui_input(window));
	}
	
	pub fn end_frame(&mut self, window: &WinitWindow) {
		self.output = self.ctx.end_frame();
		self.winit.handle_platform_output(window, &self.ctx, std::mem::take(&mut self.output.platform_output));
	}
	
	pub fn ctx(&self) -> &Context {
		&self.ctx
	}
	
	pub fn paint(&mut self, window: &WinitWindow, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<bool, WindowGuiPaintError> {
		let result = self.painter.update_textures(std::mem::take(&mut self.output.textures_delta), builder)?;
		let wait_for_frame = result == UpdateTexturesResult::Changed;
		
		let extend = self.framebuffer.extent();
		let viewport = Viewport {
			origin: [0.0, 0.0],
			dimensions: [extend[0] as f32, extend[1] as f32],
			depth_range: 0.0..1.0,
		};
		
		builder.begin_render_pass(RenderPassBeginInfo {
			                         clear_values: vec![None],
			                         ..RenderPassBeginInfo::framebuffer(self.framebuffer.clone())
		                         }, SubpassContents::Inline)?
		       .set_viewport(0, Some(viewport.clone()));
		
		let size = window.inner_size();
		let sf = window.scale_factor() as f32;
		self.painter.draw(builder,
		                  [(size.width as f32) / sf, (size.height as f32) / sf],
		                  &self.ctx,
		                  std::mem::take(&mut self.output.shapes),
		                  viewport)?;
		
		builder.end_render_pass()?;
		
		Ok(wait_for_frame)
	}
}


#[derive(Debug, Error)]
pub enum WindowGuiError {
	#[error(display = "{}", _0)] RenderPassCreationError(#[error(source)] render_pass::RenderPassCreationError),
	#[error(display = "{}", _0)] PainterCreationError(#[error(source)] egui_vulkano::PainterCreationError),
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] FramebufferCreationError(#[error(source)] render_pass::FramebufferCreationError),
}

#[derive(Debug, Error)]
pub enum WindowGuiRegenFramebufferError {
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] FramebufferCreationError(#[error(source)] render_pass::FramebufferCreationError),
}

#[derive(Debug, Error)]
pub enum WindowGuiPaintError {
	#[error(display = "{}", _0)] RenderPassError(#[error(source)] command_buffer::RenderPassError),
	#[error(display = "{}", _0)] UpdateTexturesError(#[error(source)] egui_vulkano::UpdateTexturesError),
	#[error(display = "{}", _0)] DrawError(#[error(source)] egui_vulkano::DrawError),
}
