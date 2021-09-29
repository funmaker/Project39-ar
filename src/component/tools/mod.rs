mod spawner;
mod remover;

use crate::application::Application;
use crate::component::toolgun::ToolGun;

pub fn get_all_tools() -> Vec<Box<dyn Tool>> {
	vec![
		Box::new(spawner::Spawner::new()),
		Box::new(remover::Remover::new()),
	]
}

pub trait Tool {
	fn name(&self) -> &str;
	fn tick(&mut self, _toolgun: &ToolGun, _application: &Application) {}
}


