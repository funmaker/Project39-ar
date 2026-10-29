use std::error::Error as StdError;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::{Instant, Duration};
use anyhow::{Result, Error};
use egui::Key;
use thiserror::Error;
use simba::scalar::SubsetOf;
use vulkano::{command_buffer, swapchain, sync, Validated, VulkanError};
use vulkano::command_buffer::{AutoCommandBufferBuilder, BlitImageInfo, CommandBufferUsage, ImageBlit, ImageCopy};
use vulkano::format::ClearValue;
use vulkano::image::{Image, ImageSubresourceLayers};
use vulkano::image::sampler::Filter;
use vulkano::swapchain::{Surface, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo, SwapchainPresentInfo};
use vulkano::sync::GpuFuture;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::error::ExternalError;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent, MouseButton, DeviceEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::window::{WindowAttributes, Fullscreen, CursorGrabMode, Window as WinitWindow, WindowId};

mod gui;

use crate::config;
use crate::math::{Isometry3, Perspective3, projective_clip, Vec2, PI, Mat4, PMat4};
use crate::renderer::{Renderer, RenderTarget, RenderContext, RenderTargetContext};
use crate::utils::FramebufferBundle;
use super::Input;
use gui::WindowGui;


const FOV: f32 = 110.0;

pub struct Window {
	pub inv_proj: Option<StereoProjection>,
	pub window: Arc<WinitWindow>,
	last_present: Instant,
	swapchain: Arc<Swapchain>,
	swapchain_images: Vec<Arc<Image>>,
	swapchain_regen_needed: bool,
	acquire_image_num: Option<u32>,
	acquire_future: Option<SwapchainAcquireFuture>,
	fb: FramebufferBundle,
	render_required: bool,
	cursor_trapped: bool,
	gui: WindowGui,
}

impl Window {
	pub fn new(window: impl Into<Arc<WinitWindow>>, event_loop: &ActiveEventLoop, renderer: &Renderer) -> Result<Window> {
		let window = window.into();
		
		fn into_vec(ps: PhysicalSize<u32>) -> Vec2 {
			vector!(ps.width as f32, ps.height as f32)
		}
		
		let outer_size = into_vec(window.outer_size());
		let monitor_size = window.current_monitor()
		                         .map(|mon| into_vec(mon.size()))
		                         .unwrap_or(outer_size);
		let centered_pos = (monitor_size - outer_size) / 2.0;
		
		if centered_pos.x >= 0.0 && centered_pos.y >= 0.0 {
			window.set_outer_position(PhysicalPosition::new(centered_pos.x, centered_pos.y));
		}
		
		let surface = Surface::from_window(renderer.instance.clone(), window.clone())?;
		let (swapchain, swapchain_images) = renderer.create_swapchain(surface.clone())?;
		
		let swapchain_extent = swapchain.image_extent();
		let fb = renderer.create_framebuffer((swapchain_extent[0], swapchain_extent[1]))?;
		let gui = WindowGui::new(&fb, event_loop, surface.clone(), renderer)?;
		
		Ok(Window {
			window,
			swapchain,
			swapchain_images,
			swapchain_regen_needed: false,
			acquire_image_num: None,
			acquire_future: None,
			fb,
			last_present: Instant::now(),
			render_required: true,
			cursor_trapped: false,
			gui,
			inv_proj: None,
		})
	}
	
	pub fn regen_swapchain(&mut self, renderer: &Renderer) -> Result<()> {
		if !self.swapchain_regen_needed {
			return Ok(())
		}
		
		let window_size = self.window.inner_size();
		let framebuffer_size = (window_size.width, window_size.height);
		
		if window_size.width == 0 || window_size.height == 0 {
			self.swapchain_regen_needed = false;
			return Err(WindowSwapchainNeedRetry.into())
		}
		
		let (swapchain, swapchain_images) = self.swapchain.recreate(SwapchainCreateInfo {
			                                                  image_extent: [framebuffer_size.0, framebuffer_size.1],
			                                                  ..self.swapchain.create_info()
		                                                  })?;
		
		self.swapchain = swapchain;
		self.swapchain_images = swapchain_images;
		
		self.fb = renderer.create_framebuffer(framebuffer_size)?;
		self.gui.regen_framebuffer(&self.fb)?;
		
		self.swapchain_regen_needed = false;
		
		Ok(())
	}
	
	pub fn acquire_swapchain_image(&mut self) -> Result<Option<(u32, SwapchainAcquireFuture)>> {
		let timeout = if !self.render_required {
			let max_fps = config::get().window_max_fps;
			
			if max_fps != 0 && self.last_present.elapsed().as_secs_f32() < 1.0 / max_fps as f32 {
				return Ok(None);
			} else {
				Some(Duration::new(0, 0))
			}
		} else {
			None
		};
		
		let (image_num, suboptimal, acquire_future) = match swapchain::acquire_next_image(self.swapchain.clone(), timeout) {
			Err(Validated::Error(VulkanError::OutOfDate)) => {
				self.swapchain_regen_needed = true;
				return Ok(None);
			},
			Err(Validated::Error(VulkanError::Timeout)) => {
				return Ok(None);
			},
			Err(err) => return Err(err.into()),
			Ok(res) => res,
		};
		
		if suboptimal {
			eprintln!("WARN: Suboptimal window swapchain!");
			self.swapchain_regen_needed = true;
		}
		
		Ok(Some((image_num, acquire_future)))
	}
	
	pub fn mirror_from(&mut self,
	                   rt: impl RenderTarget,
	                   context: Option<&RenderTargetContext>,
	                   renderer: &mut Renderer)
	                   -> Result<()> {
		let (swapchain_img_num, acquire_future) = match self.acquire_swapchain_image()? {
			Some(result) => result,
			None => return Ok(()),
		};
		
		let src_img = rt.last_frame();
		let src_extent = src_img.extent();
		let mid_img = &self.fb.main_image;
		let mid_extent = mid_img.extent();
		let dst_img = &self.swapchain_images[swapchain_img_num as usize];
		let layers = src_img.array_layers();
		
		let mut builder = AutoCommandBufferBuilder::primary(renderer.command_buffer_allocator.clone(),
		                                                    renderer.queue.queue_family_index(),
		                                                    CommandBufferUsage::OneTimeSubmit)?;
		
		let mut copy_info = BlitImageInfo::images(src_img.clone(), mid_img.clone());
		copy_info.filter = Filter::Linear;
		copy_info.regions.clear();
		
		for layer in 0..layers {
			copy_info.regions.push(ImageBlit {
				src_subresource: ImageSubresourceLayers {
					array_layers: layer..(layer + 1),
					..mid_img.subresource_layers()
				},
				dst_subresource: ImageSubresourceLayers {
					array_layers: 0..1,
					..mid_img.subresource_layers()
				},
				src_offsets: [
					[0, 0, 0],
					[src_extent[0], src_extent[1], src_extent[2]]
				],
				dst_offsets: [
					[mid_extent[0] / layers * layer, 0, 0],
					[mid_extent[0] / layers * (layer + 1), mid_extent[1], 1]
				],
				..ImageBlit::default()
			});
		}
		
		builder.blit_image(copy_info)?;
		
		self.gui.paint(&mut builder)?;
		
		builder.blit_image(BlitImageInfo {
			filter: Filter::Nearest,
			..BlitImageInfo::images(mid_img.clone(), dst_img.clone())
		})?;
		
		let command_buffer = builder.build()?;
		
		self.render_required = false;
		self.last_present = Instant::now();
		
		let queue = renderer.queue.clone();
		renderer.try_enqueue(queue.clone(), |future| {
			Ok(
				future.join(acquire_future)
				      .then_execute(queue.clone(), command_buffer)?
				      .then_swapchain_present(queue.clone(), SwapchainPresentInfo::swapchain_image_index(self.swapchain.clone(), swapchain_img_num))
				      .boxed()
			)
		})?;
		
		if let Some(context) = context {
			self.inv_proj = Some(StereoProjection::stereo(context));
		}
		
		Ok(())
	}
	
	pub fn cursor_trapped(&self) -> bool {
		self.cursor_trapped
	}
	
	pub fn grab_cursor(&mut self, grab: bool) -> Result<()> {
		self.cursor_trapped = grab;
		self.window.set_cursor_visible(!grab);
		self.window.set_cursor_grab(if grab { CursorGrabMode::Confined } else { CursorGrabMode::None })?;
		Ok(())
	}
	
	pub fn start_gui_frame(&mut self) -> egui::Context {
		self.gui.start_frame();
		
		self.gui.ctx().clone()
	}
	
	pub fn end_gui_frame(&mut self) {
		self.gui.end_frame();
	}
	
	pub fn on_window_event(&mut self, event: WindowEvent, input: &mut Input, renderer: &mut Renderer) -> Result<()> {
		if !self.cursor_trapped {
			if self.gui.on_event(&event) {
				// Event consumed by GUI
				return Ok(());
			}
		}
		
		match event {
			WindowEvent::Resized(_) => self.regen_swapchain(renderer)?,
			
			WindowEvent::KeyboardInput {
				event: KeyEvent {
					physical_key: PhysicalKey::Code(KeyCode::Escape),
					state: ElementState::Pressed,
					repeat: false, ..
				}, ..
			} if self.cursor_trapped => {
				self.grab_cursor(false)?;
			}
			
			WindowEvent::KeyboardInput {
				event: KeyEvent {
					physical_key: PhysicalKey::Code(KeyCode::KeyF),
					state: ElementState::Pressed,
					repeat: false, ..
				}, ..
			} if self.cursor_trapped => {
				if self.window.fullscreen().is_none() {
					self.window.set_fullscreen(Some(Fullscreen::Borderless(self.window.current_monitor())));
				} else {
					self.window.set_fullscreen(None);
				}
			}
			
			WindowEvent::MouseInput {
				button: MouseButton::Left,
				state: ElementState::Pressed, ..
			} if !self.cursor_trapped => {
				self.grab_cursor(true)?;
				
				let size = self.window.inner_size();
				let center = PhysicalPosition::new(size.width as f32 / 2.0, size.height as f32 / 2.0);
				self.window.set_cursor_position(center)?;
				
				// When cursor is grabbed, egui stops receiving events. Let's fake button release so it doesn't think it's constantly pressed.
				let mut fake_event = event.clone();
				if let WindowEvent::MouseInput { state, .. } = &mut fake_event {
					*state = ElementState::Released;
				}
				self.gui.on_event(&fake_event);
			}
			
			WindowEvent::KeyboardInput {
				event: KeyEvent {
					physical_key: PhysicalKey::Code(code),
					state,
					repeat: false, ..
				}, ..
			} if self.cursor_trapped => {
				input.keyboard.update_button(code, state == ElementState::Pressed);
			}
			
			WindowEvent::MouseInput {
				button,
				state, ..
			} if self.cursor_trapped => {
				input.mouse.update_button(button, state == ElementState::Pressed);
			}
			
			_ => {}
		}
		
		if self.gui.is_dragging() && self.cursor_trapped {
			self.grab_cursor(false)?;
		}
		
		Ok(())
	}
	
	pub fn on_device_event(&mut self, event: DeviceEvent, input: &mut Input) -> Result<()> {
		match event {
			DeviceEvent::Motion {
				axis,
				value,
			} if self.cursor_trapped() => {
				let size = self.window.inner_size();
				let center = PhysicalPosition::new(size.width / 2, size.height / 2);
				
				input.mouse.update_axis(axis as usize, value as f32);
				
				self.window.set_cursor_position(center)?;
			},
			
			_ => {},
		}
		
		Ok(())
	}
}

impl RenderTarget for Window {
	fn create_context(&mut self, camera_pos: Isometry3) -> Result<Option<RenderTargetContext>> {
		match self.acquire_swapchain_image()? {
			None => return Ok(None),
			Some((image_num, future)) => {
				self.acquire_image_num = Some(image_num);
				self.acquire_future = Some(future);
			},
		};
		
		let view = camera_pos.inverse().to_superset();
		let framebuffer_size = self.fb.extent();
		let aspect = framebuffer_size.0 as f32 / framebuffer_size.1 as f32;
		let fovx = FOV / 180.0 * PI;
		let fovy = ((fovx / 2.0).tan() / aspect).atan() * 2.0;
		let projection = projective_clip() * Perspective3::new(aspect, fovy, 0.1, 100.0).as_projective();
		
		Ok(Some(RenderTargetContext::new(self.fb.clone(),
		                                 (view, view),
		                                 (projection, projection),
		                                 (vector!(fovx, fovy), vector!(fovx, fovy)))))
	}

	fn clear_values(&self) -> &[Option<ClearValue>] {
		&self.fb.clear_values
	}

	fn last_frame(&self) -> &Arc<Image> {
		&self.fb.main_image
	}
	
	fn after_render(&mut self, context: &mut RenderContext, renderer: &mut Renderer) -> Result<()> {
		let image_num = self.acquire_image_num.unwrap();
		let acquire_future = self.acquire_future.take().unwrap();
		
		self.gui.paint(&mut context.builder)?;
		
		renderer.enqueue(renderer.queue.clone(), |future| future.join(acquire_future).boxed());
		
		context.builder.blit_image(BlitImageInfo::images(self.last_frame().clone(), self.swapchain_images[image_num as usize].clone()))?;
		
		self.inv_proj = Some(StereoProjection::mono(context));
		
		Ok(())
	}
	
	fn after_execute(&mut self, renderer: &mut Renderer) -> Result<()> {
		let image_num = self.acquire_image_num.take().unwrap();
		
		let queue = renderer.queue.clone();
		renderer.enqueue(queue.clone(), |future| future.then_swapchain_present(queue, SwapchainPresentInfo::swapchain_image_index(self.swapchain.clone(), image_num)).boxed());
		
		Ok(())
	}
}

#[derive(Debug, Copy, Clone)]
pub enum StereoProjection {
	Mono(PMat4),
	Stereo(PMat4, PMat4),
}

impl StereoProjection {
	fn mono(context: &RenderTargetContext) -> Self {
		StereoProjection::Mono(
			(context.projection.0 * context.view.0).inverse()
		)
	}
	
	fn stereo(context: &RenderTargetContext) -> Self {
		StereoProjection::Stereo(
			(context.projection.0 * context.view.0).inverse(),
			(context.projection.1 * context.view.1).inverse(),
		)
	}
}

#[derive(Debug, Error)]
#[error("Need Retry")]
pub struct WindowSwapchainNeedRetry;
