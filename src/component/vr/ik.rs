use std::cell::Cell;
use std::time::Duration;
use egui::Ui;
use rapier3d::dynamics::{RigidBodyBuilder, RigidBodyType};
use rapier3d::prelude::{ColliderBuilder, RigidBodyHandle};

use crate::application::{Entity, Application, EntityRef, Physics};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::debug;
use crate::math::{Isometry3, Vec3, PI, to_euler, from_euler, Color, Point3, face_towards_lossy, Rot3};
use crate::utils::{ExUi, get_user_data};

// Based on Mathias Parger's
// Inverse Kinematics for Virtual Reality
// https://diglib.tugraz.at/download.php?id=5c4a48dc5a282&location=browse

#[derive(ComponentBase)]
pub struct VrIk {
	#[inner] inner: ComponentInner,
	hmd: EntityRef,
	hand_left: EntityRef,
	hand_right: EntityRef,
	rb_torso: Cell<RigidBodyHandle>,
	rb_left_upper_arm: Cell<RigidBodyHandle>,
	rb_left_lower_arm: Cell<RigidBodyHandle>,
	rb_right_upper_arm: Cell<RigidBodyHandle>,
	rb_right_lower_arm: Cell<RigidBodyHandle>,
	
	left_hand_offset: Cell<Isometry3>,
	right_hand_offset: Cell<Isometry3>,
	base_height: Cell<f32>,
	neck_offset: Cell<Vec3>,
	shoulder_offset: Cell<Vec3>,
	height_factor: Cell<f32>,
	pitch_factor: Cell<f32>,
	arm_length: Cell<f32>,
	arm_shoulder: Cell<f32>,
	arm_upper: Cell<f32>,
	shoulder_rot_mul: Cell<f32>,
	shoulder_rot_max: Cell<f32>,
}

impl VrIk {
	pub fn new(hmd: EntityRef, hand_left: EntityRef, hand_right: EntityRef) -> Self {
		VrIk {
			inner: ComponentInner::new_norender(),
			hmd,
			hand_left,
			hand_right,
			rb_torso: Cell::new(RigidBodyHandle::invalid()),
			rb_left_upper_arm: Cell::new(RigidBodyHandle::invalid()),
			rb_left_lower_arm: Cell::new(RigidBodyHandle::invalid()),
			rb_right_upper_arm: Cell::new(RigidBodyHandle::invalid()),
			rb_right_lower_arm: Cell::new(RigidBodyHandle::invalid()),
			
			left_hand_offset: Cell::new(Isometry3::translation(0.0, 0.03, 0.17)),
			right_hand_offset: Cell::new(Isometry3::translation(0.0, 0.03, 0.17)),
			base_height: Cell::new(1.7),
			neck_offset: Cell::new(Vec3::new(0.0, -0.1, 0.05)),
			shoulder_offset: Cell::new(Vec3::new(0.0, -0.1, 0.02)),
			height_factor: Cell::new(0.8),
			pitch_factor: Cell::new(0.3),
			arm_length: Cell::new(0.75),
			arm_shoulder: Cell::new(0.24),
			arm_upper: Cell::new(0.37),
			shoulder_rot_mul: Cell::new(0.5),
			shoulder_rot_max: Cell::new(0.5),
		}
	}
	
	pub fn set_hmd(&self, hmd: EntityRef) {
		self.hmd.set(hmd);
		println!("Setting HMD {:?}", self.hmd);
	}
	
	pub fn set_hand_left(&self, hand_left: EntityRef) {
		self.hand_left.set(hand_left);
		println!("Setting hand_left {:?}", self.hand_left);
	}
	
	pub fn set_hand_right(&self, hand_right: EntityRef) {
		self.hand_right.set(hand_right);
		println!("Setting hand_right {:?}", self.hand_right);
	}
	
	fn torso_pos(&self, hmd_pos: Isometry3, left_pos: Isometry3, right_pos: Isometry3) -> Isometry3 {
		let (hmd_pitch, hmd_yaw, _) = to_euler(hmd_pos.rotation);
		
		let neck_pos = hmd_pos * Point3::from(self.neck_offset.get());
		let translation = neck_pos + from_euler(0.0, hmd_yaw, 0.0) * self.shoulder_offset.get();
		
		let height = 1.0 - (hmd_pos.translation.y / self.base_height.get()).clamp(0.0, 1.0);
		let pitch_factor = (-hmd_pitch / PI * 2.0).clamp(0.0, 1.0);
		let pitch = (-height * self.height_factor.get() * PI - pitch_factor * height * self.pitch_factor.get() * PI).clamp(-PI * 0.6, PI * 0.25);
		
		let mean_dir = (left_pos.translation.vector - hmd_pos.translation.vector).xz().normalize() + (right_pos.translation.vector - hmd_pos.translation.vector).xz().normalize();
		let mut yaw = f32::atan2(-mean_dir.x, -mean_dir.y);
		
		// flip if hands are behind the head
		if f32::min((2.0 * PI) - (yaw - hmd_yaw).abs(), (yaw - hmd_yaw).abs()) > PI / 2.0 {
			yaw += PI;
		}
		
		let rotation = from_euler(pitch, yaw, 0.0);
		
		let position = Isometry3::from_parts(translation.into(), rotation);
		
		debug::draw_line(hmd_pos, neck_pos, 6.0, Color::dred());
		debug::draw_line(neck_pos, position, 6.0, Color::dred());
		
		position
	}
	
	fn shoulder_pos(&self, torso_pos: Isometry3, shoulder_offset: Isometry3, hand_pos: Isometry3, color: Color) -> Isometry3 {
		let hand_local = (torso_pos * shoulder_offset).inverse() * hand_pos;
		let arm_length = self.arm_length.get() * (1.0 - self.arm_shoulder.get());
		let forward_ratio = hand_local.translation.z / -arm_length;
		let up_ratio = hand_local.translation.y / arm_length;
		
		let yaw = if forward_ratio > 0.0 {
			((forward_ratio - 0.5) * self.shoulder_rot_mul.get()).clamp(0.0, self.shoulder_rot_max.get())
		} else {
			(forward_ratio * self.shoulder_rot_mul.get()).clamp(-self.shoulder_rot_max.get(), 0.0)
		};
		
		let roll = ((up_ratio - 0.5) * self.shoulder_rot_mul.get()).clamp(0.0, self.shoulder_rot_max.get());
		
		let sign = shoulder_offset.translation.x.signum();
		
		let position = torso_pos * from_euler(0.0, yaw * sign, roll * sign) * shoulder_offset;
		
		debug::draw_line(torso_pos, position, 6.0, color);
		
		position
	}
	
	fn arm_pos(&self, shoulder_pos: Isometry3, hand_pos: Isometry3, left: bool, color: Color) -> (Isometry3, Isometry3) {
		let upper_arm_length = self.arm_length.get() * self.arm_upper.get();
		let lower_arm_length = self.arm_length.get() * (1.0 - self.arm_upper.get() - self.arm_shoulder.get());
		let hand_local = shoulder_pos.inverse() * hand_pos * Point3::origin();
		let hand_norm = hand_local / (upper_arm_length + lower_arm_length);
		let hand_dist = hand_local.coords.magnitude();
		
		let elbow_pitch = (
			(hand_dist.powi(2) + upper_arm_length.powi(2) - lower_arm_length.powi(2))
			/ (2.0 * hand_dist * upper_arm_length)
		).clamp(-1.0, 1.0).acos();
		
		let elbow_roll = {
			let flip = if left { 1.0 } else { -1.0 };
			
			let x_factor = PI / 3.6 * f32::max(hand_norm.x * flip + 0.1, 0.0);
			
			let y_factor = PI * (hand_norm.y / -3.0 + 0.75);
			
			let z_factor = if hand_norm.y > 0.0 {
				PI / 0.7 * f32::max(0.6 + hand_norm.z, 0.0) * f32::max(hand_norm.y, 0.0)
			} else {
				PI / -1.8 * f32::max(0.6 + hand_norm.z, 0.0) * f32::max(-hand_norm.y, 0.0)
			};
			
			(x_factor + y_factor + z_factor).clamp(PI * 0.07, PI * 0.97) * flip
		};
		
		let upper_arm_pos = shoulder_pos
		                  * face_towards_lossy(hand_local.coords)
		                  * Rot3::from_axis_angle(&Vec3::z_axis(), elbow_roll)
		                  * Rot3::from_axis_angle(&Vec3::x_axis(), elbow_pitch);
		
		let elbow_pos = upper_arm_pos * point!(0.0, 0.0, -upper_arm_length);
		let lower_arm_pos = Isometry3::from_parts(elbow_pos.into(), face_towards_lossy(hand_pos * Point3::origin() - elbow_pos));
		
		debug::draw_line(upper_arm_pos, lower_arm_pos, 6.0, color.lightness(0.75));
		debug::draw_line(lower_arm_pos, hand_pos, 6.0, color);
		
		(upper_arm_pos, lower_arm_pos)
	}
}

impl Component for VrIk {
	fn start(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		let physics = &mut *application.physics.borrow_mut();
		let user_data = get_user_data(entity.id, self.id());
		
		let upper_arm_length = self.arm_length.get() * self.arm_upper.get();
		let lower_arm_length = self.arm_length.get() * (1.0 - self.arm_upper.get() - self.arm_shoulder.get());
		
		self.rb_torso.set(
			create_rb(physics,
			          user_data,
			          ColliderBuilder::cuboid(0.2, 0.25, 0.1)
			                          .translation(vector!(0.0, -0.2, 0.0))
			                          .user_data(user_data))
		);
		
		self.rb_left_upper_arm.set(
			create_rb(physics,
			          user_data,
			          ColliderBuilder::capsule_z(upper_arm_length * 0.5, 0.05)
			                          .translation(vector!(0.0, 0.0, -upper_arm_length * 0.5))
			                          .user_data(user_data))
		);
		
		self.rb_left_lower_arm.set(
			create_rb(physics,
			          user_data,
			          ColliderBuilder::capsule_z(lower_arm_length * 0.5, 0.05)
			                          .translation(vector!(0.0, 0.0, -lower_arm_length * 0.5))
			                          .user_data(user_data))
		);
		
		self.rb_right_upper_arm.set(
			create_rb(physics,
			          user_data,
			          ColliderBuilder::capsule_z(upper_arm_length * 0.5, 0.05)
			                          .translation(vector!(0.0, 0.0, -upper_arm_length * 0.5))
			                          .user_data(user_data))
		);
		
		self.rb_right_lower_arm.set(
			create_rb(physics,
			          user_data,
			          ColliderBuilder::capsule_z(lower_arm_length * 0.5, 0.05)
			                          .translation(vector!(0.0, 0.0, -lower_arm_length * 0.5))
			                          .user_data(user_data))
		);
		
		Ok(())
	}
	
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		let root_pos = *entity.state().position;
		let hmd_pos = self.hmd.get(application)
		                      .map(|hmd| *hmd.state().position)
		                      .unwrap_or_else(|| Isometry3::translation(0.0, 1.7, 0.0) * root_pos);
		let hand_left = self.hand_left.get(application)
		                              .map(|hand_left| *hand_left.state().position * self.left_hand_offset.get())
		                              .unwrap_or_else(|| Isometry3::translation(-0.5, 0.0, 0.0) * hmd_pos);
		let hand_right = self.hand_right.get(application)
		                                .map(|hand_right| *hand_right.state().position * self.right_hand_offset.get())
		                                .unwrap_or_else(|| Isometry3::translation(0.5, 0.0, 0.0) * hmd_pos);
		
		let physics = &mut *application.physics.borrow_mut();
		
		debug::draw_point(hmd_pos, 16.0, Color::red());
		debug::draw_point(hand_left, 16.0, Color::green());
		debug::draw_point(hand_right, 16.0, Color::blue());
		
		let torso_pos = self.torso_pos(hmd_pos, hand_left, hand_right);
		let shoulder_length = self.arm_length.get() * self.arm_shoulder.get();
		let shoulder_left = self.shoulder_pos(torso_pos, Isometry3::translation(-shoulder_length, 0.0, 0.0), hand_left, Color::dgreen());
		let shoulder_right = self.shoulder_pos(torso_pos, Isometry3::translation(shoulder_length, 0.0, 0.0), hand_right, Color::dblue());
		
		let (upper_left_pos, lower_left_pos) = self.arm_pos(shoulder_left, hand_left, true, Color::dgreen());
		let (upper_right_pos, lower_right_pos) = self.arm_pos(shoulder_right, hand_right, false, Color::dblue());
		
		if let Some(rb) = physics.rigid_body_set.get_mut(self.rb_torso.get())           { rb.set_position(torso_pos, true); }
		if let Some(rb) = physics.rigid_body_set.get_mut(self.rb_left_upper_arm.get())  { rb.set_position(upper_left_pos, true); }
		if let Some(rb) = physics.rigid_body_set.get_mut(self.rb_left_lower_arm.get())  { rb.set_position(lower_left_pos, true); }
		if let Some(rb) = physics.rigid_body_set.get_mut(self.rb_right_upper_arm.get()) { rb.set_position(upper_right_pos, true); }
		if let Some(rb) = physics.rigid_body_set.get_mut(self.rb_right_lower_arm.get()) { rb.set_position(lower_right_pos, true); }
		
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		ui.inspect_row("HMD", &self.hmd, application);
		ui.inspect_row("Hand Left", &self.hand_left, application);
		ui.inspect_row("Hand Right", &self.hand_right, application);
		ui.inspect_row("LHand Offset", &self.left_hand_offset, ());
		ui.inspect_row("RHand Offset", &self.right_hand_offset, ());
		ui.inspect_row("Base Height", &self.base_height, (0.01, 0.0..=3.0));
		ui.inspect_row("Base Height", &self.base_height, (0.01, 0.0..=3.0));
		ui.inspect_row("Neck Offset", &self.neck_offset, ());
		ui.inspect_row("Shoulder Offset", &self.shoulder_offset, ());
		ui.inspect_row("Height Factor", &self.height_factor, (0.01, -2.0..=2.0));
		ui.inspect_row("HMD Pitch Factor", &self.pitch_factor, (0.01, -2.0..=2.0));
		ui.inspect_row("Arm Length", &self.arm_length, (0.01, 0.0..=1.0));
		ui.inspect_row("Shoulder/Arm Ratio", &self.arm_shoulder, (0.01, 0.0..=1.0));
		ui.inspect_row("UpperArm/Arm Ratio", &self.arm_upper, (0.01, 0.0..=1.0));
		ui.inspect_row("Shoulder Rot Mul", &self.shoulder_rot_mul, (0.01, 0.0..=1.0));
		ui.inspect_row("Shoulder Rot Max", &self.shoulder_rot_max, (0.01, 0.0..=1.0));
	}
	
	fn on_inspect_extra(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		ui.inspect_collapsing()
		  .title("Torso")
		  .show(ui, self.rb_torso.get(), application);
	}
}

fn create_rb(physics: &mut Physics, user_data: u128, collider: ColliderBuilder) -> RigidBodyHandle {
	let handle = physics.rigid_body_set.insert(
		RigidBodyBuilder::new(RigidBodyType::KinematicPositionBased)
			.user_data(user_data)
			.build()
	);
	
	physics.collider_set.insert_with_parent(
		collider,
		handle,
		&mut physics.rigid_body_set,
	);
	
	handle
}
