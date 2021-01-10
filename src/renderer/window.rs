use std::sync::Arc;
use std::error::Error;
use err_derive::Error;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent, MouseButton};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, Fullscreen};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::platform::run_return::EventLoopExtRunReturn;
use vulkano::swapchain::{self, Surface, Swapchain, SwapchainCreationError, AcquireError};
use vulkano::image::{SwapchainImage, AttachmentImage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, BlitImageError, BuildError, CommandBufferExecError};
use vulkano::device::{Queue, Device};
use vulkano::sampler::Filter;
use vulkano::sync::GpuFuture;
use vulkano::{format, OomError};
use vulkano_win::{VkSurfaceBuild, CreationError};
use winit::window::Window as WinitWindow;

use super::{Renderer, RendererSwapchainError};
use crate::debug::{set_debug_flag, get_debug_flag};


pub struct Window {
	event_loop: EventLoop<()>,
	surface: Arc<Surface<WinitWindow>>,
	pub swapchain: (Arc<Swapchain<WinitWindow>>, Vec<Arc<SwapchainImage<WinitWindow>>>),
	pub swapchain_regen_required: bool,
	pub render_required: bool,
	pub quit_required: bool,
	pub cursor_trap: bool,
}

impl Window {
	pub fn new(renderer: &Renderer) -> Result<Window, WindowCreationError> {
		let event_loop = EventLoop::new();
		
		let surface = WindowBuilder::new().with_transparent(true)
		                                  .with_inner_size(PhysicalSize::new(1024, 768))
		                                  .with_title("Project 39")
		                                  .build_vk_surface(&event_loop, renderer.instance.clone())?;
		
		let window = surface.window();
		let size = window.outer_size();
		let monitor_size = window.current_monitor().map(|mon| mon.size()).unwrap_or(size);
		
		window.set_outer_position(PhysicalPosition::new((monitor_size.width - size.width) / 2, (monitor_size.height - size.height) / 2));
		
		let swapchain = renderer.create_swapchain(surface.clone())?;
		
		Ok(Window {
			event_loop,
			surface,
			swapchain,
			swapchain_regen_required: false,
			render_required: true,
			quit_required: false,
			cursor_trap: false,
		})
	}
	
	pub fn regen_swapchain(&mut self) -> Result<(), WindowSwapchainRegenError> {
		let dimensions = self.surface.window().inner_size().into();
		
		self.swapchain = self.swapchain.0.recreate_with_dimensions(dimensions)
		                     .map_err(|err| match err {
			                     SwapchainCreationError::UnsupportedDimensions => WindowSwapchainRegenError::NeedRetry, // No idea why this happens on linux
			                     err => err.into(),
		                     })?;
		
		Ok(())
	}
	
	pub fn render(&mut self,
	              device: &Arc<Device>,
	              queue: &Arc<Queue>,
	              future: Box<dyn GpuFuture>,
	              left: &Arc<AttachmentImage<format::R8G8B8A8Srgb>>,
	              right: &Arc<AttachmentImage<format::R8G8B8A8Srgb>>)
	              -> Result<Box<dyn GpuFuture>, WindowRenderError> {
		let (ref mut swapchain, ref mut images) = self.swapchain;
		
		let (image_num, suboptimal, acquire_future) = match swapchain::acquire_next_image(swapchain.clone(), None) {
			Err(AcquireError::OutOfDate) => {
				self.swapchain_regen_required = true;
				Err(WindowRenderError::NeedRetry)
			},
			Err(err) => Err(err.into()),
			Ok(res) => Ok(res),
		}?;
		
		if suboptimal {
			eprintln!("Suboptimal");
			self.swapchain_regen_required = true;
		}
		
		if image_num > 2 {
			eprintln!("Acquire_next_image returned {}! Skipping render.", image_num);
			self.swapchain_regen_required = true;
			return Err(WindowRenderError::NeedRetry);
		}
		
		let out_dims = swapchain.dimensions();
		let left_dims = left.dimensions();
		let right_dims = right.dimensions();
		
		let mut builder = AutoCommandBufferBuilder::new(device.clone(), queue.family().clone())?;
		
		builder.blit_image(left.clone(),
		                   [0, 0, 0],
		                   [left_dims[0] as i32, left_dims[1] as i32, 1],
		                   0,
		                   0,
		                   images[image_num].clone(),
		                   [0, 0, 0],
		                   [out_dims[0] as i32 / 2, out_dims[1] as i32, 1],
		                   0,
		                   0,
		                   1,
		                   Filter::Linear)?
		       .blit_image(right.clone(),
		                   [0, 0, 0],
		                   [right_dims[0] as i32, right_dims[1] as i32, 1],
		                   0,
		                   0,
		                   images[image_num].clone(),
		                   [out_dims[0] as i32 / 2, 0, 0],
		                   [out_dims[0] as i32, out_dims[1] as i32, 1],
		                   0,
		                   0,
		                   1,
		                   Filter::Linear)?;
		
		let command_buffer = builder.build()?;
		
		Ok(Box::new(future.join(acquire_future)
		                  .then_execute(queue.clone(), command_buffer)?
		                  .then_swapchain_present(queue.clone(), swapchain.clone(), image_num)))
	}
	
	pub fn pull_events(&mut self) {
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
								state: ElementState::Pressed, ..
							}, ..
						}, ..
					} if is_cursor_trapped => {
						match code {
							VirtualKeyCode::Q => {
								*quit_required = true;
								*control_flow = ControlFlow::Exit;
							},
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
							code => set_debug_flag(&format!("Key{:?}", code), true),
						}
					}
					
					Event::WindowEvent {
						event: WindowEvent::KeyboardInput {
							input: KeyboardInput {
								virtual_keycode: Some(code),
								state: ElementState::Released, ..
							}, ..
						}, ..
					} if is_cursor_trapped => {
						set_debug_flag(&format!("Key{:?}", code), false)
					}
					
					Event::WindowEvent {
						event: WindowEvent::MouseInput {
							button: MouseButton::Left,
							state: ElementState::Pressed, ..
						}, ..
					} => {
						let window = surface.window();
						let size = window.inner_size();
						let center = PhysicalPosition::new(size.width as f32 / 2.0, size.height as f32 / 2.0);
						
						grab_cursor(true)?;
						window.set_cursor_position(center)?;
					}
					
					Event::WindowEvent {
						event: WindowEvent::CursorMoved { position, .. }, ..
					} if is_cursor_trapped => {
						let window = surface.window();
						let size = window.inner_size();
						let center = PhysicalPosition::new((size.width as f32 / 2.0).floor(), (size.height as f32 / 2.0).floor());
						
						let cur_move = get_debug_flag("mouse_move").unwrap_or((0.0_f32, 0.0_f32));
						set_debug_flag("mouse_move", (cur_move.0 + position.x as f32 - center.x, cur_move.1 + position.y as f32 - center.y));
						
						window.set_cursor_position(center)?;
					}
					
					Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
						*new_swapchain_required = true;
						*control_flow = ControlFlow::Exit;
					}
					
					Event::RedrawRequested(_) | Event::RedrawEventsCleared => {
						*render_required = true;
						*control_flow = ControlFlow::Exit;
					},
					
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
	#[error(display = "{}", _0)] SwapchainCreationError(#[error(source)] SwapchainCreationError),
}

#[derive(Debug, Error)]
pub enum WindowRenderError {
	#[error(display = "Need Retry")] NeedRetry,
	#[error(display = "{}", _0)] AcquireError(#[error(source)] AcquireError),
	#[error(display = "{}", _0)] BlitImageError(#[error(source)] BlitImageError),
	#[error(display = "{}", _0)] OomError(#[error(source)] OomError),
	#[error(display = "{}", _0)] BuildError(#[error(source)] BuildError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] CommandBufferExecError),
}
