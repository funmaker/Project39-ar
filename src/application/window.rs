use std::sync::Arc;
use std::error::Error;
use std::time::{Instant, Duration};
use std::fmt::Debug;
use err_derive::Error;
use simba::scalar::SubsetOf;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent, MouseButton, DeviceEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, Fullscreen};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::platform::run_return::EventLoopExtRunReturn;
use winit::window::Window as WinitWindow;
use vulkano_win::{VkSurfaceBuild, CreationError};
use vulkano::{command_buffer, swapchain};
use vulkano::swapchain::{Surface, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo, SwapchainCreationError};
use vulkano::image::{AttachmentImage, SwapchainImage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage};
use vulkano::format::ClearValue;
use vulkano::sampler::Filter;
use vulkano::sync::GpuFuture;
use vulkano::image::ImageAccess;

use crate::renderer::{Renderer, RenderTarget, RendererSwapchainError};
use crate::config;
use crate::math::{Isometry3, Perspective3, projective_clip, Vec2, PI};
use crate::application::Input;
use crate::renderer::{RenderContext, RendererCreateFramebufferError, RenderTargetContext};
use crate::utils::FramebufferBundle;

const FOV: f32 = 110.0;

pub struct Window {
	event_loop: EventLoop<()>,
	surface: Arc<Surface<WinitWindow>>,
	last_present: Instant,
	swapchain: Arc<Swapchain<WinitWindow>>,
	swapchain_images: Vec<Arc<SwapchainImage<WinitWindow>>>,
	swapchain_regen_needed: bool,
	acquire_image_num: Option<usize>,
	acquire_future: Option<SwapchainAcquireFuture<WinitWindow>>,
	fb: FramebufferBundle,
	render_required: bool,
	cursor_trap: bool,
}

impl Window {
	pub fn new(size_hint: Option<(u32, u32)>, renderer: &Renderer) -> Result<Window, WindowCreationError> {
		let event_loop = EventLoop::new();
		
		let mut inner_size = size_hint.unwrap_or((1920, 960));
		inner_size.0 *= 2;
		
		if inner_size.1 > 1080 {
			inner_size.0 = (inner_size.0 as f32 * 1080.0 / inner_size.1 as f32) as u32;
			inner_size.1 = 1080;
		}
		
		let surface = WindowBuilder::new()
		                            .with_transparent(true)
		                            .with_resizable(true)
		                            .with_inner_size(PhysicalSize::new(inner_size.0, inner_size.1))
		                            .with_title("Project 39")
		                            .build_vk_surface(&event_loop, renderer.instance.clone())?;
		
		fn into_vec(ps: PhysicalSize<u32>) -> Vec2 {
			vector!(ps.width as f32, ps.height as f32)
		}
		
		let window = surface.window();
		let size = into_vec(window.outer_size());
		let monitor_size = window.current_monitor()
		                         .map(|mon| into_vec(mon.size()))
		                         .unwrap_or(size.clone());
		let centered_pos = (monitor_size - size) / 2.0;
		
		if centered_pos.x >= 0.0 && centered_pos.y >= 0.0 {
			window.set_outer_position(PhysicalPosition::new(centered_pos.x, centered_pos.y));
		}
		
		let (swapchain, swapchain_images) = renderer.create_swapchain(surface.clone())?;
		
		let swapchain_extent = swapchain.image_extent();
		let fb = renderer.create_framebuffer((swapchain_extent[0], swapchain_extent[1]))?;
		
		Ok(Window {
			event_loop,
			surface,
			swapchain,
			swapchain_images,
			swapchain_regen_needed: false,
			acquire_image_num: None,
			acquire_future: None,
			fb,
			last_present: Instant::now(),
			render_required: true,
			cursor_trap: false,
		})
	}
	
	pub fn regen_swapchain(&mut self, renderer: &Renderer) -> Result<(), WindowSwapchainRegenError> {
		if !self.swapchain_regen_needed {
			return Ok(())
		}
		
		let window_size = self.surface.window().inner_size();
		let framebuffer_size = (window_size.width, window_size.height);
		
		if window_size.width == 0 || window_size.height == 0 {
			self.swapchain_regen_needed = false;
			return Err(WindowSwapchainRegenError::NeedRetry)
		}
		
		let (swapchain, swapchain_images) = self.swapchain.recreate(SwapchainCreateInfo {
			                                                  image_extent: [framebuffer_size.0, framebuffer_size.1],
			                                                  ..self.swapchain.create_info()
		                                                  })
		                                                  .map_err(|err| match err {
			                                                  SwapchainCreationError::ImageExtentNotSupported { provided, min_supported, max_supported } => {
				                                                  eprintln!("SwapchainCreationError: ImageExtentNotSupported\n\tprovided: {:?}\n\tmin_supported: {:?}\n\tmax_supported: {:?}", provided, min_supported, max_supported);
				                                                  WindowSwapchainRegenError::NeedRetry
			                                                  }, // No idea why this happens on linux
			                                                  err => err.into(),
		                                                  })?;
		
		self.swapchain = swapchain;
		self.swapchain_images = swapchain_images;
		
		self.fb = renderer.create_framebuffer(framebuffer_size)?;
		
		self.swapchain_regen_needed = false;
		
		Ok(())
	}
	
	pub fn acquire_swapchain_image(&mut self) -> Result<Option<(usize, SwapchainAcquireFuture<WinitWindow>)>, swapchain::AcquireError> {
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
			Err(swapchain::AcquireError::OutOfDate) => {
				self.swapchain_regen_needed = true;
				return Ok(None);
			},
			Err(swapchain::AcquireError::Timeout) => {
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
	
	pub fn mark_dirty(&mut self) {
		self.render_required = true;
	}
	
	pub fn mirror_from(&mut self,
	                   image: &Arc<AttachmentImage>,
	                   renderer: &mut Renderer)
	                   -> Result<(), WindowMirrorFromError> {
		let (image_num, acquire_future) = match self.acquire_swapchain_image()? {
			Some(result) => result,
			None => return Ok(()),
		};
		
		let out_dims = self.swapchain.image_extent();
		let image_dims = image.dimensions();
		let layers = image_dims.array_layers() as i32;
		
		let mut builder = AutoCommandBufferBuilder::primary(renderer.device.clone(), renderer.queue.family().clone(), CommandBufferUsage::OneTimeSubmit)?;
		
		for layer in 0..layers {
			builder.blit_image(image.clone(),
			                   [0, 0, 0],
			                   [image_dims.width() as i32, image_dims.height() as i32, 1],
			                   layer as u32,
			                   0,
			                   self.swapchain_images[image_num].clone(),
			                   [out_dims[0] as i32 / layers * layer, 0, 0],
			                   [out_dims[0] as i32 / layers * (layer + 1), out_dims[1] as i32, 1],
			                   0,
			                   0,
			                   1,
			                   Filter::Linear)?;
		}
		
		let command_buffer = builder.build()?;
		
		self.render_required = false;
		self.last_present = Instant::now();
		
		let queue = renderer.queue.clone();
		renderer.try_enqueue::<command_buffer::CommandBufferExecError, _>(queue.clone(), |future| Ok(
			future.join(acquire_future)
			      .then_execute(queue.clone(), command_buffer)?
			      .then_swapchain_present(queue.clone(), self.swapchain.clone(), image_num)
			      .boxed()
		))?;
		
		Ok(())
	}
	
	pub fn pull_events(&mut self, input: &mut Input) {
		let surface = &self.surface;
		let new_swapchain_required = &mut self.swapchain_regen_needed;
		let render_required = &mut self.render_required;
		let is_cursor_trapped = self.cursor_trap;
		let cursor_trap = &mut self.cursor_trap;
		
		let mut grab_cursor = |grab: bool| {
			let window = surface.window();
			*cursor_trap = grab;
			window.set_cursor_visible(!grab);
			window.set_cursor_grab(grab)
		};
		
		self.event_loop.run_return(|event, _, control_flow| {
			let result: Result<(), Box<dyn Error>> = try {
				*control_flow = ControlFlow::Poll;
				
				match event {
					Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
						input.quit_required = true;
						*control_flow = ControlFlow::Exit;
					},
					
					Event::WindowEvent {
						event: WindowEvent::KeyboardInput {
							input: KeyboardInput {
								virtual_keycode: Some(code),
								state, ..
							}, ..
						}, ..
					} if is_cursor_trapped => {
						if state == ElementState::Pressed {
							match code {
								// VirtualKeyCode::Q => {
								// 	*quit_required = true;
								// 	*control_flow = ControlFlow::Exit;
								// },
								VirtualKeyCode::Escape => {
									grab_cursor(false)?;
								},
								VirtualKeyCode::F => {
									let window = surface.window();
									
									if let None = window.fullscreen() {
										window.set_fullscreen(Some(Fullscreen::Borderless(window.current_monitor())));
									} else {
										window.set_fullscreen(None);
									}
								},
								_ => {},
							}
						}
						
						if code != VirtualKeyCode::Escape {
							input.keyboard.update_button(code, state == ElementState::Pressed);
						}
					}
					
					Event::WindowEvent {
						event: WindowEvent::MouseInput {
							button: MouseButton::Left,
							state: ElementState::Pressed, ..
						}, ..
					} if !is_cursor_trapped => {
						let window = surface.window();
						let size = window.inner_size();
						let center = PhysicalPosition::new(size.width as f32 / 2.0, size.height as f32 / 2.0);
						
						grab_cursor(true)?;
						window.set_cursor_position(center)?;
					}
					
					Event::WindowEvent {
						event: WindowEvent::MouseInput {
							button,
							state, ..
						}, ..
					} if is_cursor_trapped => {
						input.mouse.update_button(button, state == ElementState::Pressed);
					}
					
					Event::DeviceEvent {
						event: DeviceEvent::Motion {
							axis,
							value,
						}, ..
					} if is_cursor_trapped => {
						let window = surface.window();
						let size = window.inner_size();
						let center = PhysicalPosition::new(size.width / 2, size.height / 2);
						
						input.mouse.update_axis(axis as usize, value as f32);
						
						window.set_cursor_position(center)?;
					}
					
					Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
						*new_swapchain_required = true;
						*control_flow = ControlFlow::Exit;
					}
					
					Event::RedrawRequested(_) => {
						*render_required = true;
						*control_flow = ControlFlow::Exit;
					},
					
					Event::RedrawEventsCleared => {
						*control_flow = ControlFlow::Exit;
					}
					
					_ => {}
				}
			};
			
			if let Err(error) = result {
				eprintln!("Error while processing events {}", error);
				input.quit_required = true;
				*control_flow = ControlFlow::Exit;
			}
		});
	}
}

impl RenderTarget for Window {
	type RenderError = WindowRenderTargetError;

	fn create_context(&mut self, camera_pos: Isometry3) -> Result<Option<RenderTargetContext>, Self::RenderError> {
		match self.acquire_swapchain_image()? {
			None => return Ok(None),
			Some((image_num, future)) => {
				self.acquire_image_num = Some(image_num);
				self.acquire_future = Some(future);
			},
		};
		
		let view = camera_pos.inverse().to_superset();
		let framebuffer_size = self.fb.size();
		let aspect = framebuffer_size.0 as f32 / framebuffer_size.1 as f32;
		let fovx = FOV / 180.0 * PI;
		let fovy = ((fovx / 2.0).tan() / aspect).atan() * 2.0;
		let projection = projective_clip() * Perspective3::new(aspect, fovy, 0.1, 100.0).as_projective();
		
		Ok(Some(RenderTargetContext::new(self.fb.clone(),
		                                 (view, view),
		                                 (projection, projection),
		                                 (vector!(fovx, fovy), vector!(fovx, fovy)))))
	}

	fn clear_values(&self) -> &[ClearValue] {
		&self.fb.clear_values
	}

	fn last_frame(&self) -> &Arc<AttachmentImage> {
		&self.fb.main_image
	}
	
	fn after_render(&mut self, context: &mut RenderContext, renderer: &mut Renderer) -> Result<(), Self::RenderError> {
		let framebuffer_size = self.fb.size();
		let swapchain_size = self.swapchain.image_extent();
		let image_num = self.acquire_image_num.unwrap();
		let acquire_future = self.acquire_future.take().unwrap();
		
		renderer.enqueue(renderer.queue.clone(), |future| future.join(acquire_future).boxed());
		
		context.builder.blit_image(self.last_frame().clone(),
		                           [0, 0, 0],
		                           [framebuffer_size.0 as i32, framebuffer_size.1 as i32, 1],
		                           0,
		                           0,
		                           self.swapchain_images[image_num].clone(),
		                           [0, 0, 0],
		                           [swapchain_size[0] as i32, swapchain_size[1] as i32, 1],
		                           0,
		                           0,
		                           1,
		                           Filter::Linear)?;
		
		Ok(())
	}
	
	fn after_execute(&mut self, renderer: &mut Renderer) -> Result<(), Self::RenderError> {
		let image_num = self.acquire_image_num.take().unwrap();
		
		let queue = renderer.queue.clone();
		renderer.enqueue(queue.clone(), |future| future.then_swapchain_present(queue, self.swapchain.clone(), image_num).boxed());
		
		Ok(())
	}
}

#[derive(Debug, Error)]
pub enum WindowCreationError {
	#[error(display = "{}", _0)] WindowBuilderError(#[error(source)] CreationError),
	#[error(display = "{}", _0)] RendererSwapchainError(#[error(source)] RendererSwapchainError),
	#[error(display = "{}", _0)] RendererCreateFramebufferError(#[error(source)] RendererCreateFramebufferError),
}

#[derive(Debug, Error)]
pub enum WindowSwapchainRegenError {
	#[error(display = "Need Retry")] NeedRetry,
	#[error(display = "{}", _0)] RendererCreateFramebufferError(#[error(source)] RendererCreateFramebufferError),
	#[error(display = "{}", _0)] SwapchainCreationError(#[error(source)] swapchain::SwapchainCreationError),
}

#[derive(Debug, Error)]
pub enum WindowMirrorFromError {
	#[error(display = "{}", _0)] AcquireError(#[error(source)] swapchain::AcquireError),
	#[error(display = "{}", _0)] BlitImageError(#[error(source)] command_buffer::BlitImageError),
	#[error(display = "{}", _0)] OomError(#[error(source)] vulkano::OomError),
	#[error(display = "{}", _0)] BuildError(#[error(source)] command_buffer::BuildError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] command_buffer::CommandBufferExecError),
}

#[derive(Debug, Error)]
pub enum WindowRenderTargetError {
	#[error(display = "{}", _0)] WindowMirrorFromError(#[error(source)] WindowMirrorFromError),
	#[error(display = "{}", _0)] AcquireError(#[error(source)] swapchain::AcquireError),
	#[error(display = "{}", _0)] BlitImageError(#[error(source)] command_buffer::BlitImageError),
}
