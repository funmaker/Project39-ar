use rapier3d::geometry::InteractionGroups;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};

use crate::application::{Hand, Application};
use crate::math::{Ray, Similarity3, Color, Rot3, Isometry3, Vec3};
use super::tool::{Tool, ToolError};
use super::ToolGun;

pub struct Spawner {
	menu_pos: Option<Isometry3>,
	prop_idx: usize,
	ghost_pos: Option<Similarity3>,
}

impl Spawner {
	pub fn new() -> Self {
		Spawner {
			menu_pos: None,
			prop_idx: 0,
			ghost_pos: None,
		}
	}
}

impl Tool for Spawner {
	fn name(&self) -> &str {
		"Spawner"
	}
	
	fn tick(&mut self, toolgun: &ToolGun, hand: Hand, ray: Ray, application: &Application) -> Result<(), ToolError> {
		let physics = &*application.physics.borrow();
		let result = physics.query_pipeline.cast_ray_and_get_normal(&physics.collider_set, &ray, 9999.0, false, InteractionGroups::all(), None);
		
		self.ghost_pos = None;
		
		if let Some((_, intersection)) = result {
			if let Some(_prop) = toolgun.prop_manager.props.get(self.prop_idx) {
				self.ghost_pos = Some(Similarity3::from_parts(
					ray.point_at(intersection.toi).into(),
					Rot3::identity(),
					1.0,
				));
			}
		}
		
		if application.input.context_btn(hand).down {
			if self.menu_pos.is_some() {
				self.menu_pos = None;
			} else {
				self.menu_pos = Some(Isometry3::face_towards(&ray.point_at(0.5), &ray.origin, &Vec3::y_axis()));
			}
		}
		
		Ok(())
	}
	
	fn render(&mut self, toolgun: &ToolGun, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ToolError> {
		if let Some(ghost_pos) = self.ghost_pos {
			if let Some(prop) = toolgun.prop_manager.props.get(self.prop_idx) {
				prop.render_impl(ghost_pos, Color::full_white().opactiy(0.25), builder)?;
			}
		}
		
		if let Some(menu_pos) = self.menu_pos {
			let size = toolgun.prop_manager.props.len();
			let row_size = (size as f32).sqrt().ceil() as usize;
			
			for (id, model) in toolgun.prop_manager.props.iter().enumerate() {
				let x = (id % row_size) as f32;
				let y = (id / row_size) as f32;
				let hsize = row_size as f32 / 2.0 - 0.5;
				let pos = vector!((x - hsize) * 0.25,
				                  (hsize - y) * 0.25,
				                  0.0);
				let size = 0.2 / model.aabb().extents().max();
				
				let transform = menu_pos * Similarity3::from_parts(pos.into(), Rot3::from_euler_angles(0.0, 0.0, 0.0), size);
				
				model.render_impl(transform, Color::full_white().opactiy(1.0), builder)?;
			}
		}
		
		Ok(())
	}
}
