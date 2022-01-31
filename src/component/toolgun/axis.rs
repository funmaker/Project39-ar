use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use rapier3d::geometry::InteractionGroups;
use rapier3d::prelude::RevoluteJoint;

use crate::component::toolgun::ToolGun;
use crate::application::{Hand, Application, EntityRef};
use crate::math::{Ray, Isometry3, Color, Similarity3, Vec3, face_towards_lossy, PI, Point3};
use crate::component::physics::joint::JointComponent;
use crate::component::toolgun::tool::ToolError;
use crate::component::model::SimpleModel;
use crate::utils::ColliderEx;
use super::tool::Tool;

pub struct Axis {
	target: EntityRef,
	target_local_pos: Isometry3,
	ghost: Option<SimpleModel>,
	ghost_pos: Option<Isometry3>,
}

impl Axis {
	pub fn new() -> Self {
		Axis {
			target: EntityRef::null(),
			target_local_pos: Isometry3::identity(),
			ghost: None,
			ghost_pos: None
		}
	}
}

impl Tool for Axis {
	fn name(&self) -> &str {
		"Axis"
	}
	
	fn tick(&mut self, toolgun: &ToolGun, hand: Hand, ray: Ray, application: &Application) -> Result<(), ToolError> {
		self.ghost_pos = None;
		
		let result = {
			let physics = &*application.physics.borrow();
			
			if let Some((c, toi)) = physics.query_pipeline.cast_ray_and_get_normal(&physics.collider_set, &ray, 9999.0, false, InteractionGroups::all(), None) {
				physics.collider_set.get(c)
				       .map(|c| (c.entity(application), toi))
			} else {
				None
			}
		};
		
		if let Some((hit_ent, intersection)) = result {
			let hit_pos = Isometry3::from_parts(
				ray.point_at(intersection.toi).into(),
				face_towards_lossy(intersection.normal),
			);
			self.ghost_pos = Some(hit_pos);
			
			if application.input.fire_btn(hand).down {
				toolgun.fire(application);
				
				let local_pos = hit_ent.state().position.inverse() * hit_pos;
				
				if let Some(target) = self.target.get(application) {
					if target != hit_ent {
						target.state_mut().position = hit_pos * self.target_local_pos.inverse();
						target.add_component(JointComponent::new(
							RevoluteJoint::new(Vec3::z_axis())
							              .local_anchor1(self.target_local_pos * Point3::origin())
							              .local_anchor2(local_pos * Point3::origin()),
							hit_ent,
						));
						
						self.target = EntityRef::null();
						self.ghost = None;
					}
				} else if let Some(ghost) = hit_ent.find_component_by_type::<SimpleModel>() {
					if hit_ent.tag("World") != Some(true) {
						self.ghost = Some(ghost.clone());
						self.target = hit_ent.as_ref();
						self.target_local_pos = local_pos * Isometry3::new(Vec3::new(0.0, 0.0, 0.02), Vec3::new(0.0, PI, 0.0));
					}
				}
			}
		}
		
		Ok(())
	}
	
	fn render(&mut self, _toolgun: &ToolGun, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ToolError> {
		if let Some(ghost) = &self.ghost {
			if let Some(ghost_pos) = self.ghost_pos {
				ghost.render_impl(Similarity3::from_isometry(ghost_pos * self.target_local_pos.inverse(), 1.0), Color::full_white().opactiy(0.25), builder)?;
			}
		}
		
		Ok(())
	}
}
