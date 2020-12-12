use std::sync::Arc;

use err_derive::Error;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, Fullscreen};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::platform::desktop::EventLoopExtDesktop;
use vulkano::instance::Instance;
use vulkano::swapchain::{Surface, Swapchain, SwapchainCreationError};
use vulkano::image::SwapchainImage;
use vulkano::format::Format;
use vulkano_win::{VkSurfaceBuild, CreationError};

use crate::renderer::{Renderer, RendererSwapchainError};

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
		// let dimensions = self.surface.window().inner_size().into();
		//
		// self.swapchain = self.swapchain.0.recreate_with_dimensions(dimensions)
		//                      .map_err(|err| match err {
		// 	                     SwapchainCreationError::UnsupportedDimensions => SwapchainRegenError::NeedRetry,
		// 	                     err => err?,
		//                      })?;
		
		Ok(())
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
