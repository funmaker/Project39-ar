use std::sync::Arc;

use err_derive::Error;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, Fullscreen};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::platform::desktop::EventLoopExtDesktop;
use vulkano::instance::Instance;
use vulkano::swapchain::Surface;
use vulkano_win::{VkSurfaceBuild, CreationError};

pub struct Window {
	event_loop: EventLoop<()>,
	surface: Arc<Surface<winit::window::Window>>,
	pub new_swapchain_required: bool,
	pub render_required: bool,
	pub quit_required: bool,
}

impl Window {
	pub fn new(instance: Arc<Instance>) -> Result<Window, WindowCreationError> {
		let event_loop = EventLoop::new();
		
		let surface = WindowBuilder::new().with_transparent(true)
		                                  .with_inner_size(PhysicalSize::new(1024, 768))
		                                  .with_title("Project 39")
		                                  .build_vk_surface(&event_loop, instance)?;
		
		let window = surface.window();
		let size = window.outer_size();
		let monitor_size = window.current_monitor().size();
		
		window.set_outer_position(PhysicalPosition::new((monitor_size.width - size.width) / 2, (monitor_size.height - size.height) / 2));
		
		Ok(Window {
			event_loop,
			surface,
			new_swapchain_required: true,
			render_required: true,
			quit_required: false,
		})
	}
	
	pub fn pull_events(&mut self) {
		let surface = &self.surface;
		let new_swapchain_required = &mut self.new_swapchain_required;
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
}
