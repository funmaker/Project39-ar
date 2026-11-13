use std::collections::HashMap;
use rand::Rng;
use rapier3d::pipeline::QueryFilter;

use crate::application::Hand;
use crate::math::{Isometry3, Point3, Ray, Rot3, Translation3, Vec3};
use crate::utils::from_user_data;
use super::super::super::model::mmd::BodyPart;
use super::{Character, Ctx, EaseBlend};
use super::look::wrap_angle;


/// Locomotion parameters of a legs state (OpenViva's `RegisterLegSpeedInfo`).
fn locomotion_info(legs: &str) -> (f32, f32) {
	let (speed, acceleration) = match legs {
		"stand_happy_loop" | "stand_locomotion_spine_stable" | "stand_happy_social1" => (1.0, 2.0),
		"stand_angry_loop" => (1.1, 2.0),
		"stand_tired_idle" => (0.95, 2.0),
		"stand_giddy_loop" => (2.4, 2.0),
		"stand_handhold_happy_pull_hard_right" | "stand_handhold_happy_pull_hard_left" => (2.4, 1.0),
		"floor_sit_locomotion_happy" | "floor_sit_locomotion_angry" => (0.6, 1.6),
		"squat_locomotion_happy" => (0.75, 0.3),
		_ => (0.0, 0.0),
	};
	(speed * 1.4, acceleration)
}

/// Converts the animation speed to blend parameters (OpenViva's `Instance.speedToAnim`).
const SPEED_TO_ANIM: f32 = 1.4;
const BALANCE_RADIUS: f32 = 0.1;
/// Seconds without ground under the pelvis before losing balance.
const LOSE_BALANCE_TIME: f32 = 0.25;
const MIN_UNBALANCE_CHECKS: i32 = 20;

/// Physics driving of the ragdoll (OpenViva's PuppetMaster setup and `Lily.Ragdoll`): every body is pinned to
/// its animated pose and muscles rotate it relative to its parent. When the character loses balance, pins are
/// released and the ragdoll falls; once it lies still on the ground, she plays a get-up animation while the
/// pins are restored.
pub struct Puppet {
	anchor_position: Point3,
	pub yaw: f32,
	/// Global pin strength (0 = ragdoll, 1 = follows the animation).
	pub pin_weight: f32,
	/// Global muscle strength.
	pub muscle_weight: f32,
	pub ground: Option<f32>,
	no_ground_time: f32,
	balance_count: i32,
	regain_timeout: f32,
	get_up: Option<String>,
	stand_up_wait: i32,
	/// Pin and muscle weights when the regain started, with the progress.
	regain: Option<(f32, f32, f32)>,
	/// Curling up on the floor: start muscle weight and progress.
	curl: Option<(f32, f32)>,
	floor_hit_anim_time: f32,
	pub constrained: bool,
	impulse: Option<(Vec3, f32)>,
	/// Horizontal velocity the hips are driven with (locomotion and impulses).
	desired_velocity: Vec3,

	// Locomotion
	pub move_target: Option<Point3>,
	pub move_stop_distance: f32,
	move_direction: Vec3,
	current_speed: f32,
	brake: EaseBlend,
	max_speed: f32,
	acceleration: f32,
	/// Point to turn towards, with time left.
	pub face_target: Option<(Point3, f32)>,
	last_floor: Option<Point3>,
	speed_history: [Vec3; 4],
	sideways_reverse: EaseBlend,
	last_sideways_flip: f32,

	/// Pin multiplier of body parts this frame.
	pub part_pin: HashMap<BodyPart, f32>,
	/// Muscle multiplier of body parts this frame.
	pub part_muscle: HashMap<BodyPart, f32>,
	/// World targets the hands reach for, with weight.
	pub hand_reach: [Option<(Point3, f32)>; 2],
}

impl Puppet {
	pub fn new(position: Point3, yaw: f32) -> Self {
		Puppet {
			anchor_position: position,
			yaw,
			pin_weight: 1.0,
			muscle_weight: 1.0,
			ground: Some(position.y),
			no_ground_time: 0.0,
			balance_count: 15,
			regain_timeout: 1.0,
			get_up: None,
			stand_up_wait: 0,
			regain: None,
			curl: None,
			floor_hit_anim_time: 0.0,
			constrained: false,
			impulse: None,
			desired_velocity: Vec3::zeros(),
			move_target: None,
			move_stop_distance: 0.5,
			move_direction: Vec3::zeros(),
			current_speed: 0.0,
			brake: EaseBlend::new(0.0),
			max_speed: 1.4,
			acceleration: 2.0,
			face_target: None,
			last_floor: None,
			speed_history: [Vec3::zeros(); 4],
			sideways_reverse: EaseBlend::new(0.0),
			last_sideways_flip: 0.0,
			part_pin: HashMap::new(),
			part_muscle: HashMap::new(),
			hand_reach: [None, None],
		}
	}

	/// Animation root on the floor.
	pub fn anchor(&self) -> Isometry3 {
		Isometry3::from_parts(Translation3::from(self.anchor_position.coords), Rot3::from_axis_angle(&Vec3::y_axis(), self.yaw))
	}

	pub fn is_regaining_balance(&self) -> bool {
		self.get_up.is_some()
	}

	pub fn is_move_active(&self) -> bool {
		self.move_target.is_some()
	}
}

fn hand_index(hand: Hand) -> usize {
	match hand {
		Hand::Left => 0,
		Hand::Right => 1,
	}
}

impl Character {
	/// Starts walking towards `target`, stopping `distance` from it.
	pub fn move_to(&mut self, target: Point3, distance: f32) {
		if self.puppet.constrained { return; }
		self.puppet.move_target = Some(target);
		self.puppet.move_stop_distance = distance;
	}

	pub fn stop_move_to(&mut self) {
		if self.puppet.move_target.take().is_some() {
			self.puppet.brake.reset(self.puppet.current_speed);
			self.puppet.brake.start(0.0, 0.2);
		}
	}

	/// Turns to face a point for a while (OpenViva's `AutonomyFaceDirection`).
	pub fn face_towards(&mut self, point: Point3, duration: f32) {
		if self.anim.body_state != "STAND" && self.anim.body_state != "FLOOR_SIT" && self.anim.body_state != "SQUAT" { return; }
		self.puppet.face_target = Some((point, duration));
	}

	/// Pushes the character for a while (OpenViva's `PlayForce`).
	pub fn play_force(&mut self, force: Vec3, duration: f32) {
		self.puppet.impulse = Some((force, duration));
	}

	fn pelvis_body(&self) -> Option<&super::Body> {
		self.bodies.get(self.pelvis)
	}

	/// Raycasts down from the pelvis for the ground, ignoring the character itself.
	fn recalculate_ground_height(&mut self, ctx: &Ctx) {
		let Some(pelvis) = self.pelvis_body() else { return };
		let mut rng = rand::thread_rng();
		let offset = vector!(rng.gen_range(-1.0..1.0), 0.0, rng.gen_range(-1.0..1.0)) * BALANCE_RADIUS;
		let hips = self.retarget.rest_position(pelvis.bone);
		let origin = pelvis.position * (pelvis.rest.inverse() * Point3::from(hips)) + offset;
		let velocity = pelvis.velocity;
		let length = self.retarget.hips_height + 0.2 + (velocity.x * velocity.x + velocity.z * velocity.z).min(5.7) * 0.07;

		let own = &self.own_entities;
		let predicate = |_, collider: &rapier3d::geometry::Collider| !own.contains(&from_user_data(collider.user_data).0);
		let physics = ctx.app.physics.borrow();
		let ray = Ray::new(origin.coords.into(), (-Vec3::y()).into());
		let hit = physics.query_pipeline_with_filter(QueryFilter::new().exclude_sensors().predicate(&predicate))
		                 .cast_ray(&ray, length, true);

		let old = self.puppet.ground;
		self.puppet.ground = hit.map(|(_, toi)| {
			let height = origin.y - toi;
			old.map_or(height, |old| (old + height) * 0.5)
		});
	}

	fn should_check_balance(&self) -> bool {
		if self.animator.in_transition(super::super::anim::LAYER_LEGS) {
			self.anim.check_balance_transition
		} else {
			self.anim.check_balance
		}
	}

	/// OpenViva's `FixedUpdateBalanceCheck` and the ragdoll coroutines.
	pub(super) fn update_balance(&mut self, ctx: &Ctx) {
		if self.bodies.is_empty() { return; }
		self.recalculate_ground_height(ctx);
		self.update_ragdoll_coroutines(ctx);

		if self.puppet.ground.is_some() {
			self.puppet.no_ground_time = 0.0;
		}

		// Touching the floor while falling: curl up.
		if self.anim.body_state == "OFFBALANCE" && self.puppet.ground.is_some() && !self.puppet.constrained {
			if self.puppet.get_up.is_none() && self.puppet.curl.is_none() && self.puppet.regain.is_none() {
				self.puppet.curl = Some((self.puppet.muscle_weight, 0.0));
			}
			if self.anim.current == "FALLING_LOOP" || self.anim.current == "PICKED_UP" {
				self.set_target_animation("FLOOR_CURL_LOOP");
			}
		}

		if !self.should_check_balance() { return; }

		if self.puppet.ground.is_some() {
			if !self.anim.has_balance {
				if !self.puppet.is_regaining_balance() {
					self.check_can_stand_up(ctx);
				}
				// Wait for the get-up animation to fully blend in, so the alignment uses its first pose.
				let blending = self.animator.in_transition(super::super::anim::LAYER_LEGS) || self.anim.current != *self.puppet.get_up.as_ref().unwrap_or(&self.anim.current);
				if self.puppet.stand_up_wait > 0 && !blending {
					self.puppet.stand_up_wait += 1;
					if self.puppet.stand_up_wait == 3 {
						self.begin_regain_balance(ctx);
					}
				}
			}
		} else if self.anim.has_balance || !self.is_ragdoll() {
			self.puppet.no_ground_time += ctx.dt;
			if self.puppet.no_ground_time > LOSE_BALANCE_TIME {
				self.puppet.no_ground_time = 0.0;
				self.lose_balance(ctx);
			}
		}
	}

	pub fn is_ragdoll(&self) -> bool {
		self.puppet.pin_weight < 0.999 && self.anim.body_state == "OFFBALANCE"
	}

	fn allowed_to_stand_up(&mut self) -> bool {
		if self.puppet.constrained { return false; }
		let Some(pelvis) = self.pelvis_body() else { return false };
		let stopped = pelvis.velocity.norm_squared() < 0.05;
		self.puppet.balance_count = (self.puppet.balance_count + if stopped { 1 } else { -1 }).clamp(1, MIN_UNBALANCE_CHECKS);
		stopped
	}

	fn check_can_stand_up(&mut self, ctx: &Ctx) {
		if self.allowed_to_stand_up() {
			if self.puppet.balance_count == MIN_UNBALANCE_CHECKS {
				self.puppet.regain_timeout -= ctx.dt;
				if self.puppet.regain_timeout <= 0.0 && self.puppet.get_up.is_none() {
					self.begin_standing_up();
				}
			} else if self.time - self.puppet.floor_hit_anim_time > 0.5 {
				self.puppet.floor_hit_anim_time = self.time;
				if self.torso_front().y.abs() > 0.7 {
					self.set_target_animation("FLOOR_FACE_UP_IDLE");
				} else {
					self.set_target_animation("FLOOR_CURL_LOOP");
				}
			}
		}
	}

	/// Direction the physical torso's front faces.
	fn torso_front(&self) -> Vec3 {
		self.physical_axis(BodyPart::Torso, Vec3::z())
	}

	/// A humanoid space axis of a physical body, in world space.
	fn physical_axis(&self, part: BodyPart, axis: Vec3) -> Vec3 {
		let Some(body) = self.body(part).or(self.pelvis_body()) else { return axis };
		body.position.rotation * body.rest.rotation.inverse() * (self.retarget.frame * axis)
	}

	/// OpenViva's `CalculateGetUpAnimation`.
	fn get_up_animation(&self) -> String {
		let front = self.torso_front();
		let right = -self.physical_axis(BodyPart::Torso, Vec3::x());
		if front.y > 0.707 {
			"FLOOR_FACE_UP_TO_STAND".into()
		} else if right.y > 0.707 {
			"FLOOR_FACE_SIDE_TO_STAND_RIGHT".into()
		} else if right.y < -0.707 {
			"FLOOR_FACE_SIDE_TO_STAND_LEFT".into()
		} else {
			"FLOOR_FACE_DOWN_TO_STAND".into()
		}
	}

	fn begin_standing_up(&mut self) {
		let anim = self.get_up_animation();
		self.log(format!("Standing up: {anim}"));
		self.override_body_state("OFFBALANCE");
		self.override_clear_animation_priority();
		self.set_target_animation(&anim);
		self.puppet.get_up = Some(anim);
		self.puppet.stand_up_wait = 1;
	}

	/// OpenViva's `RegainBalance` coroutine start: aligns the animation with the physical body and restores the pins.
	fn begin_regain_balance(&mut self, ctx: &Ctx) {
		// Align the yaw of the animated spine with the physical one.
		let physical = self.physical_axis(BodyPart::Torso, Vec3::y());
		let torso_bone = self.body(BodyPart::Torso).map(|b| b.bone).unwrap_or(0);
		let animated = self.puppet.anchor().rotation * self.bone_world.get(torso_bone).map_or(Rot3::identity(), |t| t.rotation) * (self.retarget.frame * Vec3::y());
		if physical.xz().norm() > 0.1 && animated.xz().norm() > 0.1 {
			let diff = wrap_angle(physical.x.atan2(physical.z) - animated.x.atan2(animated.z));
			self.log(format!("Regain: physical {:.2?} animated {:.2?} yaw diff {:.1}", physical.as_slice(), animated.as_slice(), diff.to_degrees()));
			self.puppet.yaw = wrap_angle(self.puppet.yaw + diff);
		}

		self.play_sfx("get_up", 1.0, ctx);
		self.anim.speed = 0.0;
		self.puppet.curl = None;
		self.puppet.regain = Some((0.0, self.puppet.muscle_weight, 0.0));
	}

	fn update_ragdoll_coroutines(&mut self, ctx: &Ctx) {
		if let Some((start_muscle, progress)) = &mut self.puppet.curl {
			*progress = (*progress + ctx.dt / 0.5).min(1.0);
			self.puppet.muscle_weight = *start_muscle + (0.8 - *start_muscle) * *progress;
			if *progress >= 1.0 {
				self.puppet.curl = None;
			}
		}

		if let Some((start_pin, start_muscle, progress)) = &mut self.puppet.regain {
			if *progress < 1.0 {
				*progress = (*progress + ctx.dt / 0.2).min(1.0);
				self.puppet.pin_weight = *start_pin + (1.0 - *start_pin) * progress.powi(4);
				self.puppet.muscle_weight = *start_muscle + (1.0 - *start_muscle) * *progress;
				if *progress >= 1.0 {
					self.anim.speed = 1.0;
				}
			} else if self.anim.has_balance {
				self.end_ragdoll_mode();
			}
		}
	}

	fn end_ragdoll_mode(&mut self) {
		self.log("Regained balance");
		self.puppet.regain = None;
		self.puppet.get_up = None;
		self.puppet.stand_up_wait = 0;
		self.puppet.pin_weight = 1.0;
		self.puppet.muscle_weight = 1.0;
		self.anim.speed = 1.0;
	}

	/// OpenViva's `BeginRagdollMode`.
	pub fn begin_ragdoll_mode(&mut self, muscle_weight: f32, animation: &str) {
		self.log(format!("Ragdoll ({animation})"));
		self.override_body_state("OFFBALANCE");
		self.puppet.regain_timeout = 1.0;
		self.puppet.pin_weight = 0.0;
		self.puppet.muscle_weight = muscle_weight;
		self.puppet.get_up = None;
		self.puppet.stand_up_wait = 0;
		self.puppet.regain = None;
		self.puppet.curl = None;
		self.anim.speed = 1.0;
		self.anim.has_balance = false;
		self.stop_move_to();
		self.override_clear_animation_priority();
		self.set_target_animation(animation);
		self.ragdoll_mode_began();
	}

	/// OpenViva's `LoseBalance`.
	fn lose_balance(&mut self, ctx: &Ctx) {
		self.log(format!("Lost balance (pin {:.2}, anim {})", self.puppet.pin_weight, self.anim.current));
		if self.puppet.constrained {
			self.speak_at_random_intervals("SCREAMING", 4.0, 1.0, ctx);
			self.begin_ragdoll_mode(0.05, "PICKED_UP");
		} else {
			self.speak_at_random_intervals("SCREAMING", 4.0, 1.0, ctx);
			self.begin_ragdoll_mode(0.3, "FALLING_LOOP");
		}
	}

	/// Player grabbing the character: hands start hand holding, other parts constrain her (picking up).
	pub(super) fn update_grabs(&mut self, ctx: &Ctx) {
		let was_constrained = self.puppet.constrained;
		self.puppet.constrained = self.bodies.iter().any(|b| b.grabbed.is_some() && !matches!(b.part, BodyPart::LeftHand | BodyPart::RightHand));

		if self.puppet.constrained && !was_constrained {
			self.log("Grabbed");
			self.stop_move_to();
			if self.anim.has_balance {
				self.speak("STARTLE_SHORT", false, ctx);
			}
		}

		if was_constrained && !self.puppet.constrained {
			self.log("Released");
			if !self.anim.has_balance && !self.puppet.is_regaining_balance() {
				if self.puppet.ground.is_some() {
					self.begin_ragdoll_mode(0.5, "FLOOR_CURL_LOOP");
				} else {
					self.begin_ragdoll_mode(0.5, "FALLING_LOOP");
				}
			}
		}

		self.puppet.part_pin.clear();
		self.puppet.part_muscle.clear();
		for body in &self.bodies {
			if body.grabbed.is_some() {
				self.puppet.part_pin.insert(body.part, 0.0);
				self.puppet.part_muscle.insert(body.part, 0.3);
			}
		}
		if self.puppet.constrained {
			// Let the player pull her around; the balance check takes over when she's lifted.
			for body in &self.bodies {
				self.puppet.part_pin.entry(body.part).or_insert(0.25);
			}
		}
	}

	pub(super) fn locomotion_on_animation_change(&mut self, new: &str) {
		let legs = self.info(new).map(|i| i.legs.clone()).unwrap_or_default();
		let (speed, acceleration) = locomotion_info(&legs);
		self.puppet.max_speed = speed;
		self.puppet.acceleration = acceleration;
	}

	/// OpenViva's `ApplyMovement`, `CalculateSmoothSpeed` and `AutonomyFaceDirection`.
	pub(super) fn update_locomotion(&mut self, ctx: &Ctx) {
		let dt = ctx.dt;
		let floor = self.floor_pos();

		// Movement towards the target.
		if let Some(target) = self.puppet.move_target {
			let mut diff = target - floor;
			diff.y = 0.0;
			let dist = diff.norm();
			if dist <= self.puppet.move_stop_distance || !self.anim.has_balance || self.puppet.max_speed <= 0.0 {
				self.stop_move_to();
			} else {
				self.puppet.move_direction = diff / dist;
				self.puppet.current_speed = (self.puppet.current_speed + self.puppet.acceleration * dt).min(self.puppet.max_speed);
				let taper = ((dist - self.puppet.move_stop_distance) / 0.2).clamp(0.0, 1.0);
				self.puppet.current_speed *= 1.0 - (1.0 - taper).powi(2);
				self.puppet.face_target = Some((floor + self.puppet.move_direction, 0.1));
			}
		} else {
			self.puppet.brake.update(dt);
			self.puppet.current_speed = self.puppet.brake.value;
		}

		let mut desired = self.puppet.move_direction * self.puppet.current_speed;
		if let Some((force, time)) = &mut self.puppet.impulse {
			desired += *force;
			*time -= dt;
			if *time <= 0.0 {
				self.puppet.impulse = None;
			}
		}
		desired.y = 0.0;
		self.puppet.desired_velocity = desired;
		
		// Turning.
		let forward = self.forward();
		let has_balance = self.anim.has_balance;
		if let Some((point, time)) = &mut self.puppet.face_target {
			*time -= dt;
			let diff = *point - floor;
			if diff.xz().norm() > 0.05 && has_balance {
				let angle = wrap_angle(forward.x.atan2(forward.z) - diff.x.atan2(diff.z));
				let max_step = 255f32.to_radians() * dt;
				// Rotating the anchor by positive yaw turns forward from +Z towards +X.
				self.puppet.yaw = wrap_angle(self.puppet.yaw - (angle * 5.0 * dt).clamp(-max_step, max_step));
			}
			if *time <= 0.0 {
				self.puppet.face_target = None;
			}
		}

		// Animation speed parameters from the measured movement.
		let velocity = self.puppet.last_floor.map_or(Vec3::zeros(), |last| (floor - last) / dt) * SPEED_TO_ANIM;
		self.puppet.last_floor = Some(floor);
		let mut info = vector!(velocity.x, velocity.z, 1.0 + velocity.xz().norm());
		for i in 0..self.puppet.speed_history.len() - 1 {
			self.puppet.speed_history[i] = self.puppet.speed_history[i + 1];
			info += self.puppet.speed_history[i];
		}
		info /= self.puppet.speed_history.len() as f32;
		let last = self.puppet.speed_history.len() - 1;
		self.puppet.speed_history[last] = info;
		let smooth = self.puppet.speed_history[0];

		let world_velocity = vector!(smooth.x, 0.0, smooth.y);
		let mut sideways = (-world_velocity.dot(&self.left())).clamp(-1.0, 1.0);
		let forward = world_velocity.dot(&self.forward()).clamp(-1.0, 1.0);
		if self.time - self.puppet.last_sideways_flip > 0.5 {
			self.puppet.last_sideways_flip = self.time;
			self.puppet.sideways_reverse.start(if forward < 0.0 { 1.0 } else { 0.0 }, 0.5);
		}
		self.puppet.sideways_reverse.update(dt);
		sideways *= 1.0 - self.puppet.sideways_reverse.value * 2.0;

		self.animator.set_param("sidewaysSpeed", sideways);
		self.animator.set_param("forwardSpeed", forward);
		self.animator.set_param("speed", smooth.z);
	}

	/// Moves the animation root (OpenViva's `UpdateRootTransform`): it follows the physical pelvis, but while
	/// standing it is only dragged along when the body is pushed away from it, so that she stands still.
	pub(super) fn update_anchor(&mut self, ctx: &Ctx) {
		let Some(pelvis) = self.pelvis_body() else { return };
		let hips = self.bones_hips();
		let anchor_rot = Rot3::from_axis_angle(&Vec3::y_axis(), self.puppet.yaw);
		let hips_offset = anchor_rot * self.bone_skin.get(hips).map_or(Vec3::zeros(), |s| s.translation.vector);
		let pelvis_pos = pelvis.position.translation.vector;

		let follow = Point3::from(pelvis_pos - hips_offset);
		
		let mut position = if self.anim.has_balance && self.puppet.pin_weight > 0.5 {
			let mut position = self.puppet.anchor_position + self.puppet.desired_velocity * ctx.dt;
			let mut diff = follow - position;
			diff.y = 0.0;
			const SLACK: f32 = 0.08;
			if diff.norm() > SLACK {
				position += diff * (1.0 - SLACK / diff.norm());
			}
			position
		} else {
			follow
		};
		
		// While pinned (standing or getting up), the root stays on the ground (OpenViva's `StandOnGround`).
		if self.puppet.pin_weight > 0.5 && let Some(ground) = self.puppet.ground {
			position.y = ground;
		}
		self.puppet.anchor_position = position;
	}

	fn bones_hips(&self) -> usize {
		self.pelvis_body().map_or(0, |b| b.bone)
	}

	/// Drives the bodies towards their animated poses (PuppetMaster pins and muscles).
	pub(super) fn drive_bodies(&mut self, ctx: &Ctx) {
		let dt = (ctx.dt * ctx.app.physics_params.borrow().dt_mul).max(0.001);
		let anchor = self.puppet.anchor();
		let pin_weight = self.puppet.pin_weight;
		let muscle_weight = self.puppet.muscle_weight;

		let targets: Vec<Isometry3> = self.bodies.iter().map(|body| {
			let mut target = anchor * self.bone_skin.get(body.bone).copied().unwrap_or_else(Isometry3::identity) * body.rest;
			let reach = match body.part {
				BodyPart::LeftHand => self.puppet.hand_reach[hand_index(Hand::Left)],
				BodyPart::RightHand => self.puppet.hand_reach[hand_index(Hand::Right)],
				_ => None,
			};
			if let Some((point, weight)) = reach {
				target.translation.vector = target.translation.vector.lerp(&point.coords, weight);
			}
			target
		}).collect();

		for id in 0..self.bodies.len() {
			let body = self.bodies[id].clone();
			let target = targets[id];
			let part_pin = self.puppet.part_pin.get(&body.part).copied().unwrap_or(1.0);
			let part_muscle = self.puppet.part_muscle.get(&body.part).copied().unwrap_or(1.0);
			let pin = (pin_weight * part_pin).clamp(0.0, 1.0);
			let muscle = (muscle_weight * part_muscle).clamp(0.0, 1.0);

			let position = body.position;
			let mut velocity = body.velocity;
			let mut angular = body.angular_velocity;

			// A spring towards the target, frame rate independent: the old `error / dt` controller
			// became stiffer (and overshot) the higher the frame rate was.
			let error = target.translation.vector - position.translation.vector;
			let desired = clamp_norm(error * PIN_STIFFNESS, MAX_PIN_SPEED);
			let linear_response = 1.0 - (-PIN_RESPONSE * dt).exp();
			velocity += (desired - velocity) * pin * linear_response;

			// Rotation: world space pin blended with a muscle relative to the parent body.
			let world_desired = (target.rotation * position.rotation.inverse()).scaled_axis() * ANG_STIFFNESS;
			let desired = match body.parent {
				Some(parent) => {
					let parent_body = &self.bodies[parent];
					let relative = targets[parent].rotation.inverse() * target.rotation;
					let muscle_target = parent_body.position.rotation * relative;
					let muscle_desired = (muscle_target * position.rotation.inverse()).scaled_axis() * ANG_STIFFNESS
					                   + parent_body.angular_velocity;
					muscle_desired.lerp(&world_desired, pin)
				},
				None => world_desired,
			};
			let strength = if body.parent.is_some() { pin.max(muscle * 0.5) } else { pin };
			let angular_response = 1.0 - (-ANG_RESPONSE * dt).exp();
			angular += (clamp_norm(desired, MAX_MUSCLE_SPEED) - angular) * strength * angular_response;

			self.bodies[id].target = target;

			if let Some(entity) = body.entity.get(ctx.app) {
				let mut state = entity.state_mut();
				*state.velocity = velocity;
				*state.angular_velocity = angular;
			}
		}
	}

	/// Called when ragdoll mode starts.
	fn ragdoll_mode_began(&mut self) {
		self.stop_headpat_silently();
		self.stop_hug_silently();
		self.handhold_clear();
	}
}

/// Position spring stiffness, in 1/s.
const PIN_STIFFNESS: f32 = 25.0;
/// How quickly body velocities follow the pin, in 1/s.
const PIN_RESPONSE: f32 = 30.0;
/// Rotation spring stiffness, in 1/s.
const ANG_STIFFNESS: f32 = 20.0;
/// How quickly body angular velocities follow the muscles, in 1/s.
const ANG_RESPONSE: f32 = 25.0;
/// Maximum correction speed of pins, in m/s. Keeps big pose differences from yanking the ragdoll.
const MAX_PIN_SPEED: f32 = 6.0;
/// Maximum angular speed of muscles, in rad/s.
const MAX_MUSCLE_SPEED: f32 = 20.0;

fn clamp_norm(v: Vec3, max: f32) -> Vec3 {
	let norm = v.norm();
	if norm > max { v * (max / norm) } else { v }
}
