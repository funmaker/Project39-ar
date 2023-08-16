use std::collections::HashMap;
use std::fmt::{Display, Formatter, Debug};
use openvr::{TrackedDeviceIndex, ControllerState};

mod device;
mod state;

pub use device::InputDevice;
pub use state::InputState;


pub type Key = winit::event::VirtualKeyCode;
pub type MouseButton = winit::event::MouseButton;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Hand {
	Left,
	Right,
}

#[derive(Debug)]
pub struct Input {
	pub keyboard: InputDevice<Key>,
	pub mouse: InputDevice<MouseButton>,
	pub controllers: HashMap<TrackedDeviceIndex, InputDevice<usize>>,
	pub controller_state: HashMap<TrackedDeviceIndex, ControllerState>,
	pub controller_left: Option<TrackedDeviceIndex>,
	pub controller_right: Option<TrackedDeviceIndex>,
	pub quitting: bool,
}

impl Input {
	pub fn new() -> Self {
		Input {
			keyboard: InputDevice::new(false),
			mouse: InputDevice::new(true),
			controllers: HashMap::new(),
			controller_state: HashMap::new(),
			controller_left: None,
			controller_right: None,
			quitting: false,
		}
	}
	
	pub fn controller(&self, hand: Hand) -> Option<&InputDevice<usize>> {
		match hand {
			Hand::Left => self.controller_left.and_then(|id| self.controllers.get(&id)),
			Hand::Right => self.controller_right.and_then(|id| self.controllers.get(&id)),
		}
	}
	
	pub fn reset(&mut self) {
		self.keyboard.reset();
		self.mouse.reset();
		self.controllers.values_mut().for_each(InputDevice::reset);
	}
	
	pub fn fire_btn(&self, hand: Hand) -> InputState {
		self.controller(hand).map(|c| c.state(33)).unwrap_or_default() |
		self.mouse.state(MouseButton::Left)
	}
	
	pub fn use_btn(&self, hand: Hand) -> InputState {
		self.controller(hand).map(|c| c.state(2)).unwrap_or_default() |
		self.mouse.state(MouseButton::Right) |
		self.keyboard.state(Key::E)
	}
	
	pub fn use2_btn(&self, hand: Hand) -> InputState {
		self.controller(hand).map(|c| c.state(32)).unwrap_or_default() |
		self.keyboard.state(Key::Q)
	}
	
	pub fn use3_btn(&self, hand: Hand) -> InputState {
		self.controller(hand).map(|c| c.state(1)).unwrap_or_default() |
		self.mouse.state(MouseButton::Middle) |
		self.keyboard.state(Key::G)
	}
	
	pub fn set_controller_id(&mut self, hand: Hand, idx: TrackedDeviceIndex) {
		match hand {
			Hand::Left => self.controller_left = Some(idx),
			Hand::Right => self.controller_right = Some(idx),
		}
	}
	
	pub fn update_controller(&mut self, idx: TrackedDeviceIndex, state: ControllerState) {
		let previous = self.controller_state.insert(idx, state);
		let pressed = state.button_pressed & !previous.map_or(0, |s| s.button_pressed);
		let released = !state.button_pressed & previous.map_or(0, |s| s.button_pressed);
		
		let device = match self.controllers.get_mut(&idx) {
			Some(device) => device,
			None => {
				self.controllers.insert(idx, InputDevice::new(false));
				self.controllers.get_mut(&idx).unwrap()
			}
		};
		
		for bit in 0..64 {
			if pressed & (1 << bit) != 0 {
				device.update_button(bit, true);
			}
			
			if released & (1 << bit) != 0 {
				device.update_button(bit, false);
			}
		}
		
		for (id, axis) in state.axis.iter().enumerate() {
			device.update_axis(id * 2 + 0, axis.x);
			device.update_axis(id * 2 + 1, axis.y);
		}
	}
}

impl Display for Input {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		writeln!(f,
		         "Keyboard:{}\nMouse:{}\nControllers:\n{}",
		         self.keyboard,
		         self.mouse,
		         self.controllers.iter().map(|(key, val)| format!("{} ->{}\n", key, val)).collect::<String>())
	}
}
