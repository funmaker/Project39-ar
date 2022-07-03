use std::sync::Arc;
use std::error::Error;
use std::time::{Instant, Duration};
use std::fmt::{Debug, Formatter};
use err_derive::Error;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent, MouseButton, DeviceEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, Fullscreen};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::platform::run_return::EventLoopExtRunReturn;
use winit::window::Window as WinitWindow;
use vulkano_win::{VkSurfaceBuild, CreationError};
use vulkano::{command_buffer, swapchain};
use vulkano::swapchain::{AcquireError, Surface, Swapchain, SwapchainCreateInfo, SwapchainCreationError};
use vulkano::image::{SwapchainImage, AttachmentImage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage};
use vulkano::device::{Queue, Device};
use vulkano::sampler::Filter;
use vulkano::sync::GpuFuture;
use vulkano::image::ImageAccess;

use super::{Renderer, RendererSwapchainError};
use crate::config;
use crate::math::Vec2;
use crate::application::Input;


pub struct Window {
	event_loop: EventLoop<()>,
	surface: Arc<Surface<WinitWindow>>,
	last_present: Instant,
	pub swapchain: (Arc<Swapchain<WinitWindow>>, Vec<Arc<SwapchainImage<WinitWindow>>>),
	pub swapchain_regen_required: bool,
	pub render_required: bool,
	pub quit_required: bool,
	pub cursor_trap: bool,
}

impl Window {
	pub fn new(renderer: &Renderer) -> Result<Window, WindowCreationError> {
		let event_loop = EventLoop::new();
		
		let mut inner_size = renderer.eyes.frame_buffer_size;
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
		
		let swapchain = renderer.create_swapchain(surface.clone())?;
		
		Ok(Window {
			event_loop,
			surface,
			swapchain,
			last_present: Instant::now(),
			swapchain_regen_required: false,
			render_required: true,
			quit_required: false,
			cursor_trap: false,
		})
	}
	
	pub fn regen_swapchain(&mut self) -> Result<(), WindowSwapchainRegenError> {
		let image_extent = self.surface.window().inner_size().into();
		
		self.swapchain = self.swapchain.0
		                     .recreate(SwapchainCreateInfo {
			                     image_extent,
			                     ..self.swapchain.0.create_info()
		                     })
		                     .map_err(|err| match err {
			                     SwapchainCreationError::ImageExtentNotSupported { provided, min_supported, max_supported } => {
				                     eprintln!("SwapchainCreationError: ImageExtentNotSupported\n\tprovided: {:?}\n\tmin_supported: {:?}\n\tmax_supported: {:?}", provided, min_supported, max_supported);
				                     WindowSwapchainRegenError::NeedRetry
			                     }, // No idea why this happens on linux
			                     err => err.into(),
		                     })?;
		
		self.swapchain_regen_required = false;
		
		Ok(())
	}
	
	pub fn render(&mut self,
	              device: &Arc<Device>,
	              queue: &Arc<Queue>,
	              future: Box<dyn GpuFuture>,
	              image: &Arc<AttachmentImage>)
	              -> Result<Box<dyn GpuFuture>, WindowRenderError> {
		let (ref mut swapchain, ref mut images) = self.swapchain;
		
		let timeout = if !self.render_required {
			let max_fps = config::get().window_max_fps;
			
			if max_fps != 0 && self.last_present.elapsed().as_secs_f32() < 1.0 / max_fps as f32 {
				return Err(WindowRenderError::Later(future));
			} else {
				Some(Duration::new(0, 0))
			}
		} else {
			None
		};
		
		let (image_num, suboptimal, acquire_future) = match swapchain::acquire_next_image(swapchain.clone(), timeout) {
			Err(AcquireError::OutOfDate) => {
				self.swapchain_regen_required = true;
				return Err(WindowRenderError::Later(future))
			},
			Err(AcquireError::Timeout) => {
				return Err(WindowRenderError::Later(future))
			},
			Err(err) => return Err(err.into()),
			Ok(res) => res,
		};
		
		if suboptimal {
			eprintln!("Suboptimal");
			self.swapchain_regen_required = true;
		}
		
		let out_dims = swapchain.image_extent();
		let image_dims = image.dimensions();
		
		let mut builder = AutoCommandBufferBuilder::primary(device.clone(), queue.family().clone(), CommandBufferUsage::OneTimeSubmit)?;
		
		builder.blit_image(image.clone(),
		                   [0, 0, 0],
		                   [image_dims.width() as i32, image_dims.height() as i32, 1],
		                   0,
		                   0,
		                   images[image_num].clone(),
		                   [0, 0, 0],
		                   [out_dims[0] as i32 / 2, out_dims[1] as i32, 1],
		                   0,
		                   0,
		                   1,
		                   Filter::Linear)?
		       .blit_image(image.clone(),
		                   [0, 0, 0],
		                   [image_dims.width() as i32, image_dims.height() as i32, 1],
		                   1,
		                   0,
		                   images[image_num].clone(),
		                   [out_dims[0] as i32 / 2, 0, 0],
		                   [out_dims[0] as i32, out_dims[1] as i32, 1],
		                   0,
		                   0,
		                   1,
		                   Filter::Linear)?;
		
		let command_buffer = builder.build()?;
		
		self.render_required = false;
		self.last_present = Instant::now();
		
		Ok(Box::new(future.join(acquire_future)
		                  .then_execute(queue.clone(), command_buffer)?
		                  .then_swapchain_present(queue.clone(), swapchain.clone(), image_num)))
	}
	
	pub fn pull_events(&mut self, input: &mut Input) {
		let surface = &self.surface;
		let new_swapchain_required = &mut self.swapchain_regen_required;
		let render_required = &mut self.render_required;
		let quit_required = &mut self.quit_required;
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
						*quit_required = true;
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
				*quit_required = true;
				*control_flow = ControlFlow::Exit;
			}
		});
	}
}

#[derive(Debug, Error)]
pub enum WindowCreationError {
	#[error(display = "{}", _0)] WindowBuilderError(#[error(source)] CreationError),
	#[error(display = "{}", _0)] RendererSwapchainError(#[error(source)] RendererSwapchainError),
}

#[derive(Debug, Error)]
pub enum WindowSwapchainRegenError {
	#[error(display = "Need Retry")] NeedRetry,
	#[error(display = "{}", _0)] SwapchainCreationError(#[error(source)] swapchain::SwapchainCreationError),
}

#[derive(Error)]
pub enum WindowRenderError {
	#[error(display = "Later")] Later(Box<dyn GpuFuture>),
	#[error(display = "{}", _0)] AcquireError(#[error(source)] swapchain::AcquireError),
	#[error(display = "{}", _0)] BlitImageError(#[error(source)] command_buffer::BlitImageError),
	#[error(display = "{}", _0)] OomError(#[error(source)] vulkano::OomError),
	#[error(display = "{}", _0)] BuildError(#[error(source)] command_buffer::BuildError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] command_buffer::CommandBufferExecError),
}

impl Debug for WindowRenderError {
	fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
		match &self {
			WindowRenderError::Later(_)                      => f.debug_tuple("Later").field(&"GpuFuture").finish(),
			WindowRenderError::AcquireError(inner)           => f.debug_tuple("AcquireError").field(inner).finish(),
			WindowRenderError::BlitImageError(inner)         => f.debug_tuple("BlitImageError").field(inner).finish(),
			WindowRenderError::OomError(inner)               => f.debug_tuple("OomError").field(inner).finish(),
			WindowRenderError::BuildError(inner)             => f.debug_tuple("BuildError").field(inner).finish(),
			WindowRenderError::CommandBufferExecError(inner) => f.debug_tuple("CommandBufferExecError").field(inner).finish(),
		}
	}
}
