use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;

use super::state::InputState;


/// Button and axis state of a single input device (keyboard, mouse or VR controller).
#[derive(Debug)]
pub struct InputDevice<Key> {
	pub buttons: HashMap<Key, InputState>,
	/// Axis values by axis id. For incremental devices, this is the movement accumulated this frame.
	pub axis: HashMap<usize, f32>,
	/// If set, axis updates are relative and get summed up and reset every frame (eg. mouse movement).
	incremental: bool,
}

impl<Key: Copy + Hash + Eq> InputDevice<Key> {
	pub fn new(incremental: bool,) -> Self {
		InputDevice {
			buttons: HashMap::new(),
			axis: HashMap::new(),
			incremental,
		}
	}
	
	/// Clears per-frame state (`down`/`up` edges and incremental axes). Called at the end of every frame.
	pub fn reset(&mut self) {
		self.buttons.values_mut().for_each(InputState::reset);
		
		if self.incremental {
			self.axis.values_mut().for_each(|axis| *axis = 0.0);
		}
	}
	
	/// Records a button press or release.
	pub fn update_button(&mut self, key: Key, pressed: bool) {
		let button = match self.buttons.get_mut(&key) {
			Some(device) => device,
			None => {
				self.buttons.insert(key, InputState::new());
				self.buttons.get_mut(&key).unwrap()
			}
		};
		
		button.update(pressed);
	}
	
	/// Records an axis value, accumulating it for incremental devices.
	pub fn update_axis(&mut self, id: usize, state: f32) {
		if self.incremental {
			let previous = self.axis.get(&id).copied().unwrap_or_default();
			self.axis.insert(id, previous + state);
		} else {
			self.axis.insert(id, state);
		}
	}
	
	/// Returns `true` if the button was pressed this frame.
	pub fn down(&self, key: Key) -> bool {
		self.buttons.get(&key).map(|s| s.down).unwrap_or_default()
	}
	
	/// Returns `true` if the button was released this frame.
	pub fn up(&self, key: Key) -> bool {
		self.buttons.get(&key).map(|s| s.up).unwrap_or_default()
	}
	
	/// Returns `true` while the button is held down.
	pub fn pressed(&self, key: Key) -> bool {
		self.buttons.get(&key).map(|s| s.pressed).unwrap_or_default()
	}
	
	/// Returns a value that flips on every press of the button.
	pub fn toggle(&self, key: Key) -> bool {
		self.buttons.get(&key).map(|s| s.toggle).unwrap_or_default()
	}
	
	pub fn state(&self, key: Key) -> InputState {
		self.buttons.get(&key).cloned().unwrap_or(InputState::new())
	}
	
	pub fn axis(&self, id: usize) -> f32 {
		self.axis.get(&id).copied().unwrap_or_default()
	}
}


impl<Key: Debug> Display for InputDevice<Key> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f,
		       "{}{}",
		       self.buttons.iter().filter(|(_, s)| s.pressed).map(|(k, _)| format!(" {:?}", k)).collect::<String>(),
		       self.axis.iter().filter(|(_, s)| s.abs() > f32::EPSILON).map(|(k, s)| format!(" A{}({})", k, s)).collect::<String>())
	}
}

