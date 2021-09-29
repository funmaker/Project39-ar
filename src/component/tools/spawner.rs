use crate::component::tools::Tool;

pub struct Spawner {

}

impl Spawner {
	pub fn new() -> Self {
		Spawner {}
	}
}

impl Tool for Spawner {
	fn name(&self) -> &str {
		"Spawner"
	}
}
