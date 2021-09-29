use crate::component::tools::Tool;

pub struct Remover {

}

impl Remover {
	pub fn new() -> Self {
		Remover {}
	}
}

impl Tool for Remover {
	fn name(&self) -> &str {
		"Spawner"
	}
}
