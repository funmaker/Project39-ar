use anyhow::Result;

use crate::application::{Hand, Application};
use crate::math::Ray;
use crate::utils::ColliderEx;
use super::ToolGun;
use super::tool::Tool;


pub struct Remover;

impl Remover {
	pub fn new() -> Self {
		Remover {}
	}
}

impl Tool for Remover {
	fn name(&self) -> &str {
		"Remover"
	}
	
	fn tick(&mut self, toolgun: &ToolGun, hand: Hand, ray: Ray, application: &Application) -> Result<()> {
		if !application.input.fire_btn(hand).down {
			return Ok(());
		}
		
		toolgun.fire(application);
		
		let result = {
			let physics = &*application.physics.borrow();
			physics.query_pipeline()
			       .cast_ray(&ray, 9999.0, false)
			       .and_then(|(c, _)| physics.colliders.get(c))
			       .map(|collider| collider.entity(application))
		};
			
		if let Some(target) = result {
			if target.tag("World") != Some(true) {
				target.remove();
			}
		}
		
		Ok(())
	}
}
