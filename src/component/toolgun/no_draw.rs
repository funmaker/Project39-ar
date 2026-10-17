use anyhow::Result;
use rapier3d::pipeline::QueryFilter;

use crate::application::{Application, Hand};
use crate::component::ComponentBase;
use crate::component::model::SimpleModel;
use crate::math::Ray;
use crate::utils::ColliderEx;
use super::ToolGun;
use super::tool::Tool;


pub struct NoDrawTool;

impl NoDrawTool {
	pub fn new() -> Self {
		NoDrawTool
	}
}

impl Tool for NoDrawTool {
	fn name(&self) -> &str {
		"NoDraw"
	}
	
	fn tick(&mut self, toolgun: &ToolGun, hand: Hand, ray: Ray, application: &Application) -> Result<()> {
		let result = {
			let physics = &*application.physics.borrow();
			
			if let Some((c, toi)) = physics.query_pipeline.cast_ray_and_get_normal(&physics.rigid_body_set, &physics.collider_set, &ray, 9999.0, false, QueryFilter::new()) {
				physics.collider_set.get(c)
				       .map(|c| (c.entity(application), toi))
			} else {
				None
			}
		};
		
		if let Some((hit_ent, _)) = result {
			if application.input.fire_btn(hand).down {
				toolgun.fire(application);
				
				let mut state = hit_ent.state_mut();
				state.hidden = !state.hidden;
			}
		}
		
		Ok(())
	}
}
