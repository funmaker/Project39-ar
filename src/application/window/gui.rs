use std::convert::TryFrom;
use std::sync::Arc;
use egui::{Context, TextStyle};
use egui_winit_vulkano::{Gui, GuiConfig};
use anyhow::Result;
use thiserror::Error;
use vulkano::{command_buffer, render_pass};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, RenderPassBeginInfo, SubpassContents};
use vulkano::format::ClearValue;
use vulkano::image::ImageAccess;
use vulkano::image::view::ImageView;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass};
use vulkano::swapchain::Surface;
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;
use winit::window::Window as WinitWindow;

use crate::renderer::{IMAGE_FORMAT, Renderer};
use crate::utils::FramebufferBundle;


pub struct WindowGui {
	gui: Gui,
	render_pass: Arc<RenderPass>,
	framebuffer: Arc<Framebuffer>,
}

impl WindowGui {
	pub fn new(fb: &FramebufferBundle, event_loop: &EventLoop<()>, surface: Arc<Surface>, renderer: &Renderer) -> Result<Self> {
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
				{ color: [color], depth_stencil: {}, input: [] }
			]
		)?;
		
		let gui = Gui::new_with_subpass(&event_loop,
		                                surface,
		                                renderer.queue.clone(),
		                                render_pass.clone().first_subpass(),
		                                GuiConfig { is_overlay: true, ..GuiConfig::default() });
		{
			let mut style = (*gui.egui_ctx.style()).clone();
			let font = style.text_styles.get_mut(&TextStyle::Monospace).unwrap();
			font.size = 16.0;
			gui.egui_ctx.set_style(style);
		}
		
		{
			let mut fonts = egui::FontDefinitions::default();
			
			fonts.font_data.insert("unifont".to_owned(), egui::FontData::from_static(include_bytes!("./unifont-15.0.04.otf")));
			
			fonts.families
			     .entry(egui::FontFamily::Proportional)
			     .or_default()
			     .push("unifont".to_owned());
			
			fonts.families
			     .entry(egui::FontFamily::Monospace)
			     .or_default()
			     .insert(0, "unifont".to_owned());
			
			gui.egui_ctx.set_fonts(fonts);
		}
		
		let framebuffer = Framebuffer::new(render_pass.clone(), FramebufferCreateInfo {
			attachments: vec![ImageView::new_default(fb.main_image.clone())?],
			extent: fb.main_image.dimensions().width_height(),
			layers: 1,
			..FramebufferCreateInfo::default()
		})?;
		
		Ok(WindowGui {
			gui,
			render_pass,
			framebuffer,
		})
	}
	
	pub fn on_event(&mut self, event: &WindowEvent) -> bool {
		self.gui.update(event)
	}
	
	pub fn regen_framebuffer(&mut self, fb: &FramebufferBundle) -> Result<()> {
		self.framebuffer = Framebuffer::new(self.render_pass.clone(), FramebufferCreateInfo {
			attachments: vec![ImageView::new_default(fb.main_image.clone())?],
			extent: fb.main_image.dimensions().width_height(),
			layers: 1,
			..FramebufferCreateInfo::default()
		})?;
		
		Ok(())
	}
	
	pub fn is_dragging(&self) -> bool {
		self.gui.egui_ctx.memory(|m| m.is_anything_being_dragged())
	}
	
	pub fn start_frame(&mut self) {
		self.gui.begin_frame();
	}
	
	pub fn end_frame(&mut self) {
	
	}
	
	pub fn ctx(&self) -> &Context {
		&self.gui.egui_ctx
	}
	
	pub fn paint(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<()> {
		let cb = self.gui.draw_on_subpass_image(self.framebuffer.extent());
		
		builder.begin_render_pass(RenderPassBeginInfo {
			                          clear_values: vec![None],
			                          ..RenderPassBeginInfo::framebuffer(self.framebuffer.clone())
		                          }, SubpassContents::SecondaryCommandBuffers)?
		       .execute_commands(cb)?
		       .end_render_pass()?;
		
		Ok(())
	}
}


