use super::tool::Tool;

pub struct Axis;

impl Axis {
	pub fn new() -> Self {
		Axis {}
	}
}

impl Tool for Axis {
	fn name(&self) -> &str {
		"Axis"
	}
}
