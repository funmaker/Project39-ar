use std::sync::Arc;
use err_derive::Error;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, Fullscreen};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::platform::desktop::EventLoopExtDesktop;
use vulkano::swapchain::{self, Surface, Swapchain, SwapchainCreationError, AcquireError};
use vulkano::image::{SwapchainImage, AttachmentImage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, BlitImageError, BuildError, CommandBufferExecError};
use vulkano::device::{Queue, Device};
use vulkano::sampler::Filter;
use vulkano::sync::GpuFuture;
use vulkano::{format, OomError};
use vulkano_win::{VkSurfaceBuild, CreationError};

use super::{Renderer, RendererSwapchainError};

type WinitWindow = winit::window::Window;

pub struct Window {
	event_loop: EventLoop<()>,
	surface: Arc<Surface<WinitWindow>>,
	pub swapchain: (Arc<Swapchain<WinitWindow>>, Vec<Arc<SwapchainImage<WinitWindow>>>),
	pub swapchain_regen_required: bool,
	pub render_required: bool,
	pub quit_required: bool,
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
		let monitor_size = window.current_monitor().size();
		
		window.set_outer_position(PhysicalPosition::new((monitor_size.width - size.width) / 2, (monitor_size.height - size.height) / 2));
		
		let swapchain = renderer.create_swapchain(surface.clone())?;
		
		Ok(Window {
			event_loop,
			surface,
			swapchain,
			swapchain_regen_required: false,
			render_required: true,
			quit_required: false,
		})
	}
	
	pub fn regen_swapchain(&mut self) -> Result<(), SwapchainRegenError> {
		let dimensions = self.surface.window().inner_size().into();
		
		self.swapchain = self.swapchain.0.recreate_with_dimensions(dimensions)
		                     .map_err(|err| match err {
			                     SwapchainCreationError::UnsupportedDimensions => SwapchainRegenError::NeedRetry,
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
	              -> Result<Box<dyn GpuFuture>, RenderError> {
		let (ref mut swapchain, ref mut images) = self.swapchain;
		
		let (image_num, suboptimal, acquire_future) = match swapchain::acquire_next_image(swapchain.clone(), None) {
			Err(AcquireError::OutOfDate) => {
				self.swapchain_regen_required = true;
				Err(RenderError::NeedRetry)
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
			return Err(RenderError::NeedRetry);
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
		
		self.event_loop.run_return(|event, _, control_flow| {
			*control_flow = ControlFlow::Poll;
			
			match event {
				Event::WindowEvent { event: WindowEvent::CloseRequested, .. } |
				Event::WindowEvent {
					event: WindowEvent::KeyboardInput {
						input: KeyboardInput {
							virtual_keycode: Some(VirtualKeyCode::Q),
							state: ElementState::Pressed, ..
						}, ..
					}, ..
				} => {
					*quit_required = true;
					*control_flow = ControlFlow::Exit;
				}
				
				Event::WindowEvent {
					event: WindowEvent::KeyboardInput {
						input: KeyboardInput {
							virtual_keycode: Some(VirtualKeyCode::F),
							state: ElementState::Pressed, ..
						}, ..
					}, ..
				} => {
					let window = surface.window();
					
					if let None = window.fullscreen() {
						window.set_fullscreen(Some(Fullscreen::Borderless(window.current_monitor())));
						window.set_cursor_visible(false);
					} else {
						window.set_fullscreen(None);
						window.set_cursor_visible(true);
					}
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
		});
	}
}

#[derive(Debug, Error)]
pub enum WindowCreationError {
	#[error(display = "{}", _0)] WindowBuilderError(#[error(source)] CreationError),
	#[error(display = "{}", _0)] RendererSwapchainError(#[error(source)] RendererSwapchainError),
}

#[derive(Debug, Error)]
pub enum SwapchainRegenError {
	#[error(display = "Need Retry")] NeedRetry,
	#[error(display = "{}", _0)] SwapchainCreationError(#[error(source)] SwapchainCreationError),
}

#[derive(Debug, Error)]
pub enum RenderError {
	#[error(display = "Need Retry")] NeedRetry,
	#[error(display = "{}", _0)] AcquireError(#[error(source)] AcquireError),
	#[error(display = "{}", _0)] BlitImageError(#[error(source)] BlitImageError),
	#[error(display = "{}", _0)] OomError(#[error(source)] OomError),
	#[error(display = "{}", _0)] BuildError(#[error(source)] BuildError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] CommandBufferExecError),
}
