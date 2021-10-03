use std::ops::BitOr;

#[derive(Debug, Copy, Clone, Default)]
pub struct InputState {
	pub down: bool,
	pub up: bool,
	pub pressed: bool,
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
	
	pub fn reset(&mut self) {
		self.down = false;
		self.up = false;
	}
	
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
