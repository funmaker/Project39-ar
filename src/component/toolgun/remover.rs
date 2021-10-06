use rapier3d::geometry::InteractionGroups;

use crate::application::{Hand, Application};
use crate::math::Ray;
use crate::utils::ColliderEx;
use super::tool::{Tool, ToolError};
use super::ToolGun;

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
	
	fn tick(&mut self, toolgun: &ToolGun, hand: Hand, ray: Ray, application: &Application) -> Result<(), ToolError> {
		if !application.input.fire_btn(hand).down {
			return Ok(());
		}
		
		toolgun.fire(application);
		
		let result = {
			let physics = &*application.physics.borrow();
			physics.query_pipeline
			       .cast_ray(&physics.collider_set, &ray, 9999.0, false, InteractionGroups::all(), None)
			       .and_then(|(c, _)| physics.collider_set.get(c))
			       .map(|collider| collider.entity(application))
		};
			
		if let Some(target) = result {
			if target.tag("NoRemove") != Some(true) {
				target.remove();
			}
		}
		
		Ok(())
	}
}
