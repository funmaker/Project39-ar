use super::tool::Tool;

pub struct BallSocket;

impl BallSocket {
	pub fn new() -> Self {
		BallSocket {}
	}
}

impl Tool for BallSocket {
	fn name(&self) -> &str {
		"Ball Socket"
	}
}
