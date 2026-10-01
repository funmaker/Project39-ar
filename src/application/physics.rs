use std::time::Duration;
use nalgebra::{Quaternion, Unit};
use rapier3d::prelude::*;

use crate::debug;
use crate::math::{Color, Isometry3, Point3, Rot3, Vec3, PI};
use crate::utils::{RigidBodyEx, ColliderEx};
use super::{Application, EntityRef};

pub trait PhysicsDebug {
	fn debug_draw(&self, application: &Application);
}

impl PhysicsDebug for PhysicsWorld {
	fn debug_draw(&self, application: &Application) {
		if debug::get_flag_or_default("DebugRigidBodiesDraw") {
			debug_draw_rigidbodies(self, application);
		}
		
		if debug::get_flag_or_default("DebugCollidersDraw") {
			debug_draw_colliders(self, application);
		}
		
		if debug::get_flag_or_default("DebugJointsDraw") {
			debug_draw_joints(self, application);
		}
	}
}

fn debug_draw_rigidbodies(world: &PhysicsWorld, application: &Application) {
	let sel_rb = application.get_selection().rigid_body();
	let sel_ent = application.get_selection().entity();
	let sel_com = application.get_selection().component().entity();
	
	for (handle, rigidbody) in world.rigid_bodies() {
		let selected = handle == sel_rb
			|| sel_ent == rigidbody.entity_ref()
			|| sel_com == rigidbody.entity_ref()
			|| (sel_rb == RigidBodyHandle::invalid() && sel_ent == EntityRef::null() && sel_com == EntityRef::null());
		
		if !selected {
			continue;
		}
		
		let position = Isometry3::from(*rigidbody.position());
		let pos = position.transform_point(&Point3::origin());
		let ang = position.rotation;
		
		debug::draw_point(pos, 8.0, Color::MAGENTA);
		debug::draw_line(pos, pos + ang * Vec3::x() * 0.03, 2.0, Color::RED);
		debug::draw_line(pos, pos + ang * Vec3::y() * 0.03, 2.0, Color::GREEN);
		debug::draw_line(pos, pos + ang * Vec3::z() * 0.03, 2.0, Color::BLUE);
	}
}

fn debug_draw_colliders(world: &PhysicsWorld, application: &Application) {
	let sel_rb = application.get_selection().rigid_body();
	let sel_col = application.get_selection().collider();
	let sel_ent = application.get_selection().entity();
	let sel_com = application.get_selection().component().entity();
	
	for (handle, collider) in world.all_colliders() {
		let selected = handle == sel_col
			|| collider.parent() == Some(sel_rb)
			|| sel_ent == collider.entity_ref()
			|| sel_com == collider.entity_ref()
			|| (sel_rb == RigidBodyHandle::invalid() && sel_col == ColliderHandle::invalid() && sel_ent == EntityRef::null() && sel_com == EntityRef::null());
		let position = Isometry3::from(*collider.position());
		
		match collider.shape().as_typed_shape() {
			TypedShape::Ball(ball) => {
				debug::draw_sphere(position, ball.radius, Color::TRANSPARENT, if selected { Color::RED } else { Color::BLACK });
			},
			TypedShape::Cuboid(cuboid) => {
				debug::draw_box(position, (cuboid.half_extents * 2.0).into(), Color::TRANSPARENT, if selected { Color::MAGENTA } else { Color::BLACK });
			},
			TypedShape::Capsule(capsule) => {
				debug::draw_capsule(position.transform_point(&capsule.segment.a.into()),
				                    position.transform_point(&capsule.segment.b.into()),
				                    capsule.radius,
				                    Color::TRANSPARENT,
				                    if selected { Color::YELLOW } else { Color::BLACK });
			},
			_ => {},
		}
	}
	
	for contact in world.narrow_phase.contact_graph().interactions() {
		if let Some(pos1) = world.colliders.get(contact.collider1).map(Collider::position) {
			for manifold in &contact.manifolds {
				for point in &manifold.points {
					debug::draw_point(pos1 * point.local_p1, 8.0, Color::D_GREEN);
				}
			}
		}
	}
}

fn debug_draw_joints(world: &PhysicsWorld, application: &Application) {
	let sel_rb = application.get_selection().rigid_body();
	let sel_joint = application.get_selection().joint();
	
	for (handle, joint) in world.impulse_joints() {
		let _selected = handle == sel_joint
			|| joint.body1() == sel_rb
			|| joint.body2() == sel_rb
			|| (sel_rb != RigidBodyHandle::invalid() && sel_joint != ImpulseJointHandle::invalid());
		
		// if !selected {
		// 	continue;
		// }
		
		let rb1 = world.bodies.get(joint.body1()).unwrap();
		let rb2 = world.bodies.get(joint.body2()).unwrap();
		
		let frame1 = Isometry3::from(rb1.position() * joint.data.local_frame1);
		let frame2 = Isometry3::from(rb2.position() * joint.data.local_frame2);
		
		let scale = debug::gizmo_scale(frame2) * 0.3;
		
		let limited = joint.data.limit_axes & !joint.data.locked_axes & !joint.data.coupled_axes & JointAxesMask::ANG_AXES;
		let coupled = joint.data.limit_axes & !joint.data.locked_axes & joint.data.coupled_axes & JointAxesMask::ANG_AXES;
		
		let diff = (frame1.rotation.inverse() * frame2.rotation).imag() * frame1.rotation.dot(&frame2.rotation).signum();
		let ang_err = diff.map(f32::asin) * 2.0;
		
		debug::draw_point(frame2, 5.0, Color::D_MAGENTA);
		debug::draw_point(frame1, 8.0, Color::MAGENTA);
		
		fn arc(steps: usize, width: f32, color: Color, pos: impl Fn(f32) -> Point3) {
			let mut from = pos(0.0);
			for step in 0..steps {
				let to = pos(step as f32 / (steps - 1) as f32);
				debug::draw_line(from, to, width, color);
				from = to;
			}
		}
		
		let single_axis = |axis: Unit<Vec3>, forward: Unit<Vec3>, err: f32, limits: JointLimits<f32>, color: Color| {
			debug::draw_line(frame2, frame2 * Point3::from(Rot3::from_axis_angle(&-axis, limits.min).transform_vector(&forward.scale(scale))), 4.0, color.lightness(0.5));
			debug::draw_line(frame2, frame2 * Point3::from(Rot3::from_axis_angle(&-axis, limits.max).transform_vector(&forward.scale(scale))), 4.0, color.lightness(0.5));
			
			arc(f32::ceil(32.0 * (limits.max - limits.min) / PI / 2.0).max(2.0) as usize,
			    3.0,
			    color.lightness(0.5),
			    |t| frame2 * Point3::from(Rot3::from_axis_angle(&-axis, (limits.max - limits.min) * t + limits.min).transform_vector(&forward.scale(scale))));
			
			debug::draw_line(frame2, frame2 * Point3::from(Rot3::from_axis_angle(&-axis, err).transform_vector(&forward.scale(scale * 1.5))), 5.0, color);
		};
		
		let double_axis = |axis1: Unit<Vec3>, axis2: Unit<Vec3>, forward: Unit<Vec3>, limits: JointLimits<f32>, color: Color| {
			let forward_point = Point3::origin() + forward.scale(scale);
			
			debug::draw_line(frame2, frame2 * Rot3::from_axis_angle(&axis1, limits.max).transform_point(&forward_point), 3.0, color.lightness(0.5));
			debug::draw_line(frame2, frame2 * Rot3::from_axis_angle(&axis1, -limits.max).transform_point(&forward_point), 3.0, color.lightness(0.5));
			debug::draw_line(frame2, frame2 * Rot3::from_axis_angle(&axis2, limits.max).transform_point(&forward_point), 3.0, color.lightness(0.5));
			debug::draw_line(frame2, frame2 * Rot3::from_axis_angle(&axis2, -limits.max).transform_point(&forward_point), 3.0, color.lightness(0.5));
			
			arc(32,
			    6.0,
			    color.lightness(0.5),
			    |t| frame2 * (Rot3::from_axis_angle(&forward, t * PI * 2.0) * Rot3::from_axis_angle(&axis1, limits.max)).transform_point(&forward_point));
			
			debug::draw_line(frame1, frame1 * Point3::from(forward.scale(scale * 1.5)), 6.0, color);
		};
		
		if limited.contains(JointAxesMask::ANG_X) { single_axis(Vec3::x_axis(), -Vec3::z_axis(), ang_err.x, joint.data.limits[JointAxis::AngX as usize], Color::RED); }
		if limited.contains(JointAxesMask::ANG_Y) { single_axis(Vec3::y_axis(), -Vec3::z_axis(), ang_err.y, joint.data.limits[JointAxis::AngY as usize], Color::GREEN); }
		if limited.contains(JointAxesMask::ANG_Z) { single_axis(Vec3::z_axis(), -Vec3::x_axis(), ang_err.z, joint.data.limits[JointAxis::AngZ as usize], Color::BLUE); }
		
		if coupled.contains(JointAxesMask::ANG_X | JointAxesMask::ANG_Y) {
			double_axis(Vec3::x_axis(), Vec3::y_axis(), -Vec3::z_axis(),
			            joint.data.limits[JointAxis::AngX as usize],
			            Color::YELLOW);
		}
		if coupled.contains(JointAxesMask::ANG_Y | JointAxesMask::ANG_Z) {
			double_axis(Vec3::y_axis(), Vec3::z_axis(), Vec3::x_axis(),
			            joint.data.limits[JointAxis::AngY as usize],
			            Color::CYAN);
		}
		if coupled.contains(JointAxesMask::ANG_Z | JointAxesMask::ANG_X) {
			double_axis(Vec3::z_axis(), Vec3::x_axis(), -Vec3::y_axis(),
			            joint.data.limits[JointAxis::AngX as usize],
			            Color::MAGENTA);
		}
	}
}
