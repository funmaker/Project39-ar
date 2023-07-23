use rapier3d::dynamics::{FixedJoint, RigidBodyType};
use rapier3d::geometry::{ColliderBuilder, ColliderShape};
use rapier3d::pipeline::QueryFilter;

use crate::debug;
use crate::application::{Application, Hand, Key};
use crate::application::entity::EntityBuilder;
use crate::component::ComponentBase;
use crate::component::model::SimpleModel;
use crate::component::model::simple::asset::ObjAsset;
use crate::component::physics::joint::JointComponent;
use crate::component::thruster::{Thruster, ThrusterDirection};
use crate::component::toolgun::ToolGun;
use crate::component::toolgun::tool::ToolError;
use crate::math::{Color, face_upwards_lossy, Isometry3, Ray, Similarity3};
use crate::renderer::{RenderContext, Renderer};
use crate::utils::ColliderEx;
use super::tool::Tool;


pub struct ThrusterTool {
	direction: ThrusterDirection,
	thruster_model: SimpleModel,
	ghost_pos: Option<Isometry3>,
}

impl ThrusterTool {
	pub fn new(renderer: &mut Renderer) -> Self {
		ThrusterTool {
			direction: ThrusterDirection::Forward,
			thruster_model: renderer.load(ObjAsset::at("shapes/thruster.obj", "shapes/thruster.png")).unwrap(),
			ghost_pos: None,
		}
	}
}

impl Tool for ThrusterTool {
	fn name(&self) -> &str {
		"Thruster"
	}
	
	fn tick(&mut self, toolgun: &ToolGun, hand: Hand, ray: Ray, application: &Application) -> Result<(), ToolError> {
		self.ghost_pos = None;
		
		let (x, y) = application.input.controller(hand)
		                              .map(|input| (input.axis(0), input.axis(1)))
		                              .unwrap_or_default();
		
		if x < -0.75 || application.input.keyboard.down(Key::Left) {
			self.direction = ThrusterDirection::Left;
		} else if x > 0.75 || application.input.keyboard.down(Key::Right) {
			self.direction = ThrusterDirection::Right;
		} else if y > 0.75 || application.input.keyboard.down(Key::Up) {
			self.direction = ThrusterDirection::Forward;
		} else if y < -0.75 || application.input.keyboard.down(Key::Down) {
			self.direction = ThrusterDirection::Back;
		}
		
		let toolgun_pos = *toolgun.entity(application).state().position;
		
		let dir_icon = match self.direction {
			ThrusterDirection::Forward => "⬆",
			ThrusterDirection::Back => "⬇",
			ThrusterDirection::Left => "⮪",
			ThrusterDirection::Right => "⮫",
		};
		
		debug::draw_text(dir_icon, toolgun_pos.transform_point(&point!(0.0, 0.12, -0.03)), debug::DebugOffset::center(0.0, 0.0), 128.0, Color::FULL_WHITE);
		
		let result = {
			let physics = &*application.physics.borrow();
			
			if let Some((c, toi)) = physics.query_pipeline.cast_ray_and_get_normal(&physics.rigid_body_set, &physics.collider_set, &ray, 9999.0, false, QueryFilter::new()) {
				physics.collider_set.get(c)
				       .map(|c| (c.entity(application), toi))
			} else {
				None
			}
		};
		
		if let Some((hit_ent, intersection)) = result {
			let hit_point = ray.point_at(intersection.toi);
			let offset = self.thruster_model.aabb().mins.y - 0.02;
			
			let ghost_pos = Isometry3::from_parts(
				(hit_point - intersection.normal * offset).into(),
				face_upwards_lossy(intersection.normal),
			);
			
			self.ghost_pos = Some(ghost_pos);
			
			if application.input.fire_btn(hand).down {
				toolgun.fire(application);
				
				let local_pos = hit_ent.state().position.inverse() * ghost_pos;
				
				application.add_entity(EntityBuilder::new("Thruster")
					.rigid_body_type(RigidBodyType::Dynamic)
					.position(ghost_pos)
					.collider(ColliderBuilder::new(ColliderShape::cylinder(0.15, 0.2)).density(200.0).build())
					.component(self.thruster_model.clone())
					.component(Thruster::new(self.direction))
					.component(JointComponent::new(
						*FixedJoint::new()
						            .set_local_frame1(Isometry3::identity())
						            .set_local_frame2(local_pos),
						hit_ent,
					))
					.build()
				);
			}
		}
		
		Ok(())
	}
	
	
	fn render(&mut self, _toolgun: &ToolGun, context: &mut RenderContext) -> Result<(), ToolError> {
		if let Some(ghost_pos) = self.ghost_pos {
			self.thruster_model.render_impl(Similarity3::from_isometry(ghost_pos, 1.0), Color::FULL_WHITE.opactiy(0.25), context)?;
		}
		
		Ok(())
	}
}
