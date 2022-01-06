use rapier3d::geometry::InteractionGroups;

use crate::application::{Hand, Application, EntityRef};
use crate::component::physics::rope::Rope;
use crate::math::{Ray, Point3};
use crate::utils::ColliderEx;
use super::tool::{Tool, ToolError};
use super::ToolGun;

pub struct RopeTool {
	selected: EntityRef,
	selected_local_offset: Point3,
}

impl RopeTool {
	pub fn new() -> Self {
		RopeTool {
			selected: EntityRef::null(),
			selected_local_offset: Point3::origin(),
		}
	}
}

impl Tool for RopeTool {
	fn name(&self) -> &str {
		"Rope"
	}
	
	fn tick(&mut self, toolgun: &ToolGun, hand: Hand, ray: Ray, application: &Application) -> Result<(), ToolError> {
		if !application.input.fire_btn(hand).down {
			return Ok(());
		}
		
		toolgun.fire(application);
		
		let result = {
			let physics = &*application.physics.borrow();
			
			if let Some((c, toi)) = physics.query_pipeline.cast_ray(&physics.collider_set, &ray, 9999.0, false, InteractionGroups::all(), None) {
				physics.collider_set.get(c)
				       .map(|c| (c.entity(application), toi))
			} else {
				None
			}
		};
		
		if let Some((hit_ent, toi)) = result {
			let hit_pos = ray.point_at(toi);
			
			if let Some(selected) = self.selected.get(application) {
				let selected_hit_pos = selected.state().position.transform_point(&self.selected_local_offset);
				
				hit_ent.add_component(Rope::new(
					hit_ent.state().position.inverse_transform_point(&hit_pos),
					selected.as_ref(),
					self.selected_local_offset,
					(hit_pos - selected_hit_pos).magnitude(),
					1.0,
				));
				
				self.selected.set_null();
			} else {
				self.selected = hit_ent.as_ref();
				self.selected_local_offset = hit_ent.state().position.inverse_transform_point(&hit_pos);
			}
		}
		
		Ok(())
	}
}
