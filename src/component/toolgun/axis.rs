use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use rapier3d::geometry::InteractionGroups;
use rapier3d::prelude::RevoluteJoint;

use crate::component::toolgun::ToolGun;
use crate::application::{Hand, Application, EntityRef};
use crate::math::{Ray, Isometry3, Color, Similarity3, Vec3, face_towards_lossy, PI, Point3};
use crate::component::toolgun::tool::ToolError;
use crate::utils::ColliderEx;
use super::tool::Tool;
use super::any_model::AnyModel;
use crate::component::physics::joint::JointComponent;

pub struct Axis {
	target: EntityRef,
	target_local_pos: Isometry3,
	ghost: Option<AnyModel>,
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
						target.state_mut().position = hit_pos * self.target_local_pos;
						target.add_component(JointComponent::new(RevoluteJoint::new(
							self.target_local_pos * Point3::origin(),
							self.target_local_pos * Vec3::z_axis(),
							local_pos * Point3::origin(),
							local_pos * Vec3::z_axis(),
						),hit_ent));
						
						self.target = EntityRef::null();
						self.ghost = None;
					}
				} else if let Some(ghost) = AnyModel::find(hit_ent) {
					if hit_ent.tag("World") != Some(true) {
						self.ghost = Some(ghost);
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
