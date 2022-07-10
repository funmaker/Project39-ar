use std::time::Duration;
use nalgebra::Quaternion;
use rapier3d::prelude::*;
use crate::debug;

use crate::math::{Color, Point3, Rot3, Vec3};

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
	pub impulse_joint_set: ImpulseJointSet,
	pub multibody_joint_set: MultibodyJointSet,
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
			integration_parameters: IntegrationParameters {
				erp: 0.8,
				joint_erp: 0.5,
				..IntegrationParameters::default()
			},
			physics_pipeline: PhysicsPipeline::new(),
			query_pipeline: QueryPipeline::new(),
			island_manager: IslandManager::new(),
			broad_phase: BroadPhase::new(),
			narrow_phase: NarrowPhase::new(),
			impulse_joint_set: ImpulseJointSet::new(),
			multibody_joint_set: MultibodyJointSet::new(),
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
		                           &mut self.impulse_joint_set,
		                           &mut self.multibody_joint_set,
		                           &mut self.ccd_solver,
		                           &self.physics_hooks,
		                           &self.event_handler);
		
		self.query_pipeline.update(&self.island_manager,
		                           &self.rigid_body_set,
		                           &self.collider_set);
	}
	
	pub fn debug_draw(&self) {
		if debug::get_flag_or_default("DebugRigidBodiesDraw") {
			self.debug_draw_rigidbodies();
		}
		
		if debug::get_flag_or_default("DebugCollidersDraw") {
			self.debug_draw_colliders();
		}
		
		if debug::get_flag_or_default("DebugJointsDraw") {
			self.debug_draw_joints();
		}
	}
	
	pub fn debug_draw_rigidbodies(&self) {
		for (_, rigidbody) in self.rigid_body_set.iter() {
			
			let position = rigidbody.position();
			let pos = position.transform_point(&Point3::origin());
			let ang = position.rotation;
			
			debug::draw_point(pos, 8.0, Color::magenta());
			debug::draw_line(pos, pos + ang * Vec3::x() * 0.03, 2.0, Color::red());
			debug::draw_line(pos, pos + ang * Vec3::y() * 0.03, 2.0, Color::green());
			debug::draw_line(pos, pos + ang * Vec3::z() * 0.03, 2.0, Color::blue());
		}
	}
	
	pub fn debug_draw_colliders(&self) {
		for (_, collider) in self.collider_set.iter() {
			match collider.shape().as_typed_shape() {
				TypedShape::Ball(ball) => {
					debug::draw_sphere(*collider.position(), ball.radius, Color::transparent(), Color::red());
				},
				TypedShape::Cuboid(cuboid) => {
					debug::draw_box(*collider.position(), cuboid.half_extents * 2.0, Color::transparent(), Color::magenta());
				},
				TypedShape::Capsule(capsule) => {
					debug::draw_capsule(collider.position().transform_point(&capsule.segment.a),
					                    collider.position().transform_point(&capsule.segment.b),
					                    capsule.radius,
					                    Color::transparent(), Color::yellow());
				},
				_ => {},
			}
		}
		
		for contact in self.narrow_phase.contact_graph().interactions() {
			if let Some(pos1) = self.collider_set.get(contact.collider1)
			                                     .map(Collider::position) {
				for manifold in &contact.manifolds {
					for point in &manifold.points {
						debug::draw_point(pos1 * point.local_p1, 8.0, Color::dgreen());
					}
				}
			}
		}
	}
	
	pub fn debug_draw_joints(&self) {
		for (_, joint) in self.impulse_joint_set.iter() {
			let rb1 = self.rigid_body_set.get(joint.body1).unwrap();
			let rb2 = self.rigid_body_set.get(joint.body2).unwrap();
			
			let frame1 = rb1.position() * joint.data.local_frame1;
			let frame2 = rb2.position() * joint.data.local_frame2;
			
			debug::draw_point(frame1 * Point3::origin(), 5.0, Color::dmagenta());
			debug::draw_point(frame2 * Point3::origin(), 5.0, Color::magenta());
			
			// if joint.data.limit_axes.contains(JointAxesMask::X) {
			// 	debug::draw_line(frame1 * point!(-1.0, 0.0, 0.0), frame1 * point!(1.0, 0.0, 0.0), 1.0, Color::dred());
			// }
			// if joint.data.limit_axes.contains(JointAxesMask::Y) {
			// 	debug::draw_line(frame1 * point!(0.0, -1.0, 0.0), frame1 * point!(0.0, 1.0, 0.0), 1.0, Color::dgreen());
			// }
			// if joint.data.limit_axes.contains(JointAxesMask::Z) {
			// 	debug::draw_line(frame1 * point!(0.0, 0.0, -1.0), frame1 * point!(0.0, 0.0, 1.0), 1.0, Color::dblue());
			// }
			
			let rot = frame2.rotation / frame1.rotation;
			let dir = frame1 * Vec3::y_axis();
			let ra = rot.vector();
			let p = ra.dot(&dir) * *dir;
			let twist = Rot3::new_normalize(Quaternion::new(rot.w, p.x, p.y, p.z));
			let swing = rot * twist.conjugate();
			
			if joint.data.limit_axes.intersects(JointAxesMask::ANG_X) {
				debug::draw_line(frame1 * Point3::origin(), frame1 * Rot3::new(*Vector::x_axis() * joint.data.limits[3].min) * point!(0.0, 0.03, 0.0), 2.0, Color::dgreen());
				debug::draw_line(frame1 * Point3::origin(), frame1 * Rot3::new(*Vector::x_axis() * joint.data.limits[3].max) * point!(0.0, 0.03, 0.0), 2.0, Color::dgreen());
			}
			
			if joint.data.limit_axes.intersects(JointAxesMask::ANG_Z) {
				debug::draw_line(frame1 * Point3::origin(), frame1 * Rot3::new(*Vector::z_axis() * joint.data.limits[5].min) * point!(0.0, 0.03, 0.0), 2.0, Color::dgreen());
				debug::draw_line(frame1 * Point3::origin(), frame1 * Rot3::new(*Vector::z_axis() * joint.data.limits[5].max) * point!(0.0, 0.03, 0.0), 2.0, Color::dgreen());
			}
			
			if joint.data.limit_axes.intersects(JointAxesMask::ANG_X | JointAxesMask::ANG_Z) {
				debug::draw_line(frame1 * Point3::origin(), frame1 * point!(0.0, 0.03, 0.0), 2.0, Color::dgreen());
				debug::draw_line(frame1 * Point3::origin(), frame1.translation * swing * frame1.rotation * point!(0.0, 0.03, 0.0), 2.0, Color::green());
			}
			
			if joint.data.limit_axes.intersects(JointAxesMask::ANG_Y) {
				debug::draw_line(frame1 * Point3::origin(), frame1 * point!(0.03, 0.0, 0.0), 2.0, Color::dred());
				debug::draw_line(frame1 * Point3::origin(), frame1 * Rot3::new(*Vector::y_axis() * joint.data.limits[4].min) * point!(0.03, 0.0, 0.0), 2.0, Color::dred());
				debug::draw_line(frame1 * Point3::origin(), frame1 * Rot3::new(*Vector::y_axis() * joint.data.limits[4].max) * point!(0.03, 0.0, 0.0), 2.0, Color::dred());
				debug::draw_line(frame1 * Point3::origin(), frame1.translation * twist * frame1.rotation * point!(0.03, 0.0, 0.0), 2.0, Color::red());
			}
		}
	}
}
