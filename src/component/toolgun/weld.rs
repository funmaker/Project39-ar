use super::tool::Tool;

pub struct Weld {

}

impl Weld {
	pub fn new() -> Self {
		Weld {}
	}
}

impl Tool for Weld {
	fn name(&self) -> &str {
		"Weld"
	}
}
