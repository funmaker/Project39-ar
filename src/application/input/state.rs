use std::ops::BitOr;


/// State of a single button.
#[derive(Debug, Copy, Clone, Default)]
pub struct InputState {
	/// Button was pressed this frame.
	pub down: bool,
	/// Button was released this frame.
	pub up: bool,
	/// Button is currently held.
	pub pressed: bool,
	/// Flips on every press.
	pub toggle: bool,
}

impl InputState {
	pub fn new() -> Self {
		InputState {
			down: false,
			up: false,
			pressed: false,
			toggle: false
		}
	}
	
	/// Clears the per-frame `down` and `up` flags.
	pub fn reset(&mut self) {
		self.down = false;
		self.up = false;
	}
	
	/// Updates the state with a new pressed value, setting edge flags if it changed.
	pub fn update(&mut self, pressed: bool) {
		if self.pressed == pressed {
			return;
		}
		
		if pressed {
			self.down = true;
			self.toggle = !self.toggle;
		} else {
			self.up = true;
		}
		
		self.pressed = pressed;
	}
}

impl BitOr for InputState {
	type Output = InputState;
	
	fn bitor(self, rhs: Self) -> Self::Output {
		InputState {
			down: self.down || rhs.down,
			up: self.up || rhs.up,
			pressed: self.pressed || rhs.pressed,
			toggle: self.down ^ rhs.down
		}
	}
}
