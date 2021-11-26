use std::time::Duration;
use rapier3d::prelude::*;
use crate::debug;

use crate::math::{Color, Vec3};

pub struct Physics {
	pub rigid_body_set: RigidBodySet,
	pub collider_set: ColliderSet,
	pub gravity: Vec3,
	pub integration_parameters: IntegrationParameters,
	pub physics_pipeline: PhysicsPipeline,
	pub query_pipeline: QueryPipeline,
	pub island_manager: IslandManager,
	pub broad_phase: BroadPhase,
	pub narrow_phase: NarrowPhase,
	pub joint_set: JointSet,
	pub ccd_solver: CCDSolver,
	pub physics_hooks: (),
	pub event_handler: (),
}


impl Physics {
	pub fn new() -> Self {
		Physics {
			rigid_body_set: RigidBodySet::new(),
			collider_set: ColliderSet::new(),
			gravity: vector!(0.0, -9.81, 0.0),
			integration_parameters: IntegrationParameters::default(),
			physics_pipeline: PhysicsPipeline::new(),
			query_pipeline: QueryPipeline::new(),
			island_manager: IslandManager::new(),
			broad_phase: BroadPhase::new(),
			narrow_phase: NarrowPhase::new(),
			joint_set: JointSet::new(),
			ccd_solver: CCDSolver::new(),
			physics_hooks: (),
			event_handler: (),
		}
	}
	
	pub fn step(&mut self, delta_time: Duration) {
		self.integration_parameters.dt = delta_time.as_secs_f32();
		
		self.physics_pipeline.step(&self.gravity,
		                           &self.integration_parameters,
		                           &mut self.island_manager,
		                           &mut self.broad_phase,
		                           &mut self.narrow_phase,
		                           &mut self.rigid_body_set,
		                           &mut self.collider_set,
		                           &mut self.joint_set,
		                           &mut self.ccd_solver,
		                           &self.physics_hooks,
		                           &self.event_handler);
		
		self.query_pipeline.update(&self.island_manager,
		                           &self.rigid_body_set,
		                           &self.collider_set);
	}
	
	pub fn debug_draw_colliders(&self) {
		for (_, collider) in self.collider_set.iter() {
			match collider.shape().as_typed_shape() {
				TypedShape::Ball(ball) => {
					debug::draw_sphere(*collider.position(), ball.radius, Color::black().opactiy(0.25), Color::magenta());
				},
				TypedShape::Cuboid(cuboid) => {
					debug::draw_box(*collider.position(), cuboid.half_extents * 2.0, Color::black().opactiy(0.25), Color::magenta());
				},
				TypedShape::Capsule(capsule) => {
					debug::draw_capsule(collider.position().transform_point(&capsule.segment.a),
					                    collider.position().transform_point(&capsule.segment.b),
					                    capsule.radius,
					                    Color::black().opactiy(0.25), Color::magenta());
				},
				_ => {},
			}
		}
	}
}
