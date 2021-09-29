use std::collections::HashMap;
use std::hash::Hash;
use std::fmt::{Debug, Display, Formatter};

use super::state::InputState;

#[derive(Debug)]
pub struct InputDevice<Key> {
	pub buttons: HashMap<Key, InputState>,
	pub axis: HashMap<usize, f32>,
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
	
	pub fn reset(&mut self) {
		self.buttons.values_mut().for_each(InputState::reset);
		
		if self.incremental {
			self.axis.values_mut().for_each(|axis| *axis = 0.0);
		}
	}
	
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
	
	pub fn update_axis(&mut self, id: usize, state: f32) {
		if self.incremental {
			let previous = self.axis.get(&id).copied().unwrap_or_default();
			self.axis.insert(id, previous + state);
		} else {
			self.axis.insert(id, state);
		}
	}
	
	pub fn down(&self, key: Key) -> bool {
		self.buttons.get(&key).map(|s| s.down).unwrap_or_default()
	}
	
	pub fn up(&self, key: Key) -> bool {
		self.buttons.get(&key).map(|s| s.up).unwrap_or_default()
	}
	
	pub fn pressed(&self, key: Key) -> bool {
		self.buttons.get(&key).map(|s| s.pressed).unwrap_or_default()
	}
	
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

