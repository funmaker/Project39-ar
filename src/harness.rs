use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};
use anyhow::{Result, Error, anyhow};

use crate::application::Application;
use crate::utils::user_events::UserEvent;

pub enum ApplicationHarness {
	Created {
		proxy: EventLoopProxy<UserEvent>,
	},
	Initializing,
	Ready {
		application: Application,
	},
	Error {
		error: Error,
	},
}

impl ApplicationHarness {
	pub fn new(event_loop: &EventLoop<UserEvent>) -> Self {
		ApplicationHarness::Created {
			proxy: event_loop.create_proxy(),
		}
	}
	
	fn handle_result(&mut self, result: Result<()>, event_loop: &ActiveEventLoop) {
		if let Err(error) = result {
			*self = ApplicationHarness::Error { error };
			
			event_loop.exit();
		}
	}
}

impl ApplicationHandler<UserEvent> for ApplicationHarness {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		if let ApplicationHarness::Created { .. } = self {
			let ApplicationHarness::Created {
				proxy,
			} = std::mem::replace(self, ApplicationHarness::Initializing) else { unreachable!() };
			
			let win_attr =
				Window::default_attributes()
					.with_transparent(true)
					.with_resizable(true)
					.with_inner_size(PhysicalSize::new(1280, 720))
					.with_title("Project 39");
			
			let result = try {
				let window = event_loop.create_window(win_attr).map_err(Error::from)?;
				let application = Application::new(window, event_loop)?;
				proxy.send_event(UserEvent::Initialized(application)).map_err(|err| anyhow!("Unable to send initialization user event: {err}"))?;
			};
			
			self.handle_result(result, event_loop);
		}
	}
	
	fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
		match event {
			UserEvent::Initialized(application) => {
				*self = ApplicationHarness::Ready { application };
			}
		}
	}
	
	fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
		let ApplicationHarness::Ready { application } = self else { return };
		
		let result = application.on_window_event(event_loop, window_id, event);
		
		self.handle_result(result, event_loop);
	}
	
	fn device_event(&mut self, event_loop: &ActiveEventLoop, device_id: DeviceId, event: DeviceEvent) {
		let ApplicationHarness::Ready { application } = self else { return };
		
		let result = application.on_device_event(event_loop, device_id, event);
		
		self.handle_result(result, event_loop);
	}
	
	fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
		let ApplicationHarness::Ready { application } = self else { return };
		
		let result = application.on_about_to_wait(event_loop);
		
		self.handle_result(result, event_loop);
	}
}
