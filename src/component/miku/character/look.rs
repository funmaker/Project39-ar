use crate::application::Hand;
use crate::math::{Point3, Rot3, Vec3};
use super::super::anim::{human, Pose};
use super::{Character, Ctx, EaseBlend};


/// What the character looks at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LookTarget {
	None,
	PlayerHead,
	PlayerHand(Hand),
	Point(Point3),
}

/// OpenViva's look-at logic (`Lily.Vision`): the head, upper spine and eyes turn towards a target,
/// with per-animation blend weights ("body flags" and "eye flags").
pub struct LookAt {
	pub target: LookTarget,
	head: EaseBlend,
	spine: EaseBlend,
	eyes: EaseBlend,
	/// Smoothed head rotation offset.
	head_offset: Rot3,
	spine_angle: f32,
	/// While positive, the target is kept instead of looking around.
	awareness_timer: f32,
	/// Point leaning the head towards it (headpats), with weight.
	pub lean: Option<(Point3, f32)>,
}

const HEAD_FOLLOW: u32 = 1;
const SPINE2_FOLLOW: u32 = 2;
const PUPIL_FOLLOW: u32 = 4;

impl LookAt {
	pub fn new() -> Self {
		LookAt {
			target: LookTarget::PlayerHead,
			head: EaseBlend::new(1.0),
			spine: EaseBlend::new(1.0),
			eyes: EaseBlend::new(1.0),
			head_offset: Rot3::identity(),
			spine_angle: 0.0,
			awareness_timer: 0.0,
			lean: None,
		}
	}

	/// OpenViva's `setBodyVariables`.
	pub fn set_body_variables(&mut self, flags: u32, percent: f32, duration: f32) {
		if percent == 0.0 { return; }
		let head = if flags & HEAD_FOLLOW != 0 { percent.min(1.0) } else { 0.0 };
		let spine = if flags & SPINE2_FOLLOW != 0 { percent.min(1.0) } else { 0.0 };
		self.head.start(head, duration);
		self.spine.start(spine, duration * 1.4);
	}

	/// OpenViva's `SetEyeVariables`.
	pub fn set_eye_variables(&mut self, flags: u32, duration: f32) {
		let eyes = if flags & PUPIL_FOLLOW != 0 { 1.0 } else { 0.0 };
		self.eyes.start(eyes, duration);
	}

	pub fn set_awareness_timeout(&mut self, time: f32) {
		self.awareness_timer = time;
	}
}

impl Character {
	pub fn set_look_at(&mut self, target: LookTarget) {
		self.look.target = target;
	}

	fn look_target_position(&self, ctx: &Ctx) -> Option<Point3> {
		match self.look.target {
			LookTarget::None => None,
			LookTarget::PlayerHead => Some(Point3::from(ctx.player.head.translation.vector)),
			LookTarget::PlayerHand(hand) => ctx.player.hand(hand).map(|h| Point3::from(h.position.translation.vector)),
			LookTarget::Point(point) => Some(point),
		}
	}

	/// Returns `true` if the point is in front of the character's head and not too far.
	pub fn can_see_point(&self, point: Point3) -> bool {
		let head = self.head_position();
		let diff = point - head;
		diff.norm() < 6.0 && diff.normalize().dot(&self.forward()) > -0.2
	}

	/// Picks what to look at and turns the head, spine and eyes of `pose` towards it.
	pub(super) fn update_look_at(&mut self, ctx: &Ctx, pose: &mut Pose) {
		let dt = ctx.dt;
		self.look.head.update(dt);
		self.look.spine.update(dt);
		self.look.eyes.update(dt);

		self.look.awareness_timer -= dt;
		if self.look.awareness_timer <= 0.0 {
			let head = Point3::from(ctx.player.head.translation.vector);
			self.look.target = if self.can_see_point(head) { LookTarget::PlayerHead } else { LookTarget::None };
		}

		if !self.anim.has_balance { return; }

		let target = self.look_target_position(ctx);
		let to_character = (self.puppet.anchor().rotation * self.retarget.frame).inverse();
		let head_pos = self.head_position();
		let mut globals = vec![];

		// Upper spine yaw (OpenViva's `lookAtSpine2`).
		pose.global_rotations(&mut globals);
		let spine_target = target.map(|t| to_character * (t - head_pos)).map_or(0.0, |dir| {
			let forward = globals[human::CHEST] * Vec3::z();
			let angle = dir.x.atan2(dir.z) - forward.x.atan2(forward.z);
			wrap_angle(angle).clamp(-0.6, 0.6)
		});
		self.look.spine_angle += (spine_target - self.look.spine_angle) * (2.5 * dt).min(1.0);
		let spine_weight = ease_in_out_cubic(self.look.spine.value);
		if spine_weight > 0.0 {
			let rotation = Rot3::from_axis_angle(&Vec3::y_axis(), self.look.spine_angle * spine_weight);
			pose.rotations[human::CHEST] = globals[human::SPINE].inverse() * rotation * globals[human::CHEST];
		}

		// Head (OpenViva's `lookAtHead`).
		pose.global_rotations(&mut globals);
		let head_weight = ease_in_out_cubic(self.look.head.value) * 0.7;
		let desired = target.map(|t| to_character * (t - head_pos))
		                    .filter(|dir| dir.norm() > 0.05)
		                    .map(|dir| {
			                    let forward = globals[human::HEAD] * Vec3::z();
			                    let rotation = Rot3::rotation_between(&forward, &dir).unwrap_or(Rot3::identity());
			                    limit_angle(rotation, 40f32.to_radians())
		                    })
		                    .unwrap_or(Rot3::identity());
		self.look.head_offset = self.look.head_offset.slerp(&desired, (8.0 * dt).min(1.0));
		let mut head_global = Rot3::identity().slerp(&self.look.head_offset, head_weight) * globals[human::HEAD];

		// Headpat lean: tilt the head towards the patting hand.
		if let Some((point, weight)) = self.look.lean {
			let dir = to_character * (point - head_pos);
			let up = head_global * Vec3::y();
			if let Some(rotation) = Rot3::rotation_between(&up, &dir) {
				head_global = Rot3::identity().slerp(&limit_angle(rotation, 35f32.to_radians()), weight) * head_global;
			}
		}

		pose.rotations[human::HEAD] = globals[human::NECK].inverse() * head_global;

		// Eyes.
		pose.global_rotations(&mut globals);
		let eye_weight = self.look.eyes.value;
		if eye_weight > 0.0 && let Some(t) = target {
			let dir = to_character * (t - head_pos);
			for side in [human::Side::Left, human::Side::Right] {
				let eye = side.eye();
				let forward = globals[eye] * Vec3::z();
				if let Some(rotation) = Rot3::rotation_between(&forward, &dir) {
					let rotation = Rot3::identity().slerp(&limit_angle(rotation, 20f32.to_radians()), eye_weight);
					pose.rotations[eye] = globals[human::HEAD].inverse() * rotation * globals[eye];
				}
			}
		}
	}

	/// Procedural overrides of the pose: lip flaps while speaking and procedural finger posing
	/// (the chopsticks game and any other behavior that moves the fingers).
	pub(super) fn apply_procedural(&mut self, ctx: &Ctx, pose: &mut Pose) {
		if let Some(jaw) = human::find_morph("jawDown") {
			pose.morphs[jaw] = pose.morphs[jaw].max(self.voice.mouth * 0.6);
		}

		self.fingers.update(ctx.dt);
		self.fingers.apply(pose);
	}
}

/// Wraps an angle to -PI..PI.
pub fn wrap_angle(mut angle: f32) -> f32 {
	use std::f32::consts::PI;
	while angle > PI { angle -= 2.0 * PI; }
	while angle < -PI { angle += 2.0 * PI; }
	angle
}

fn limit_angle(rotation: Rot3, max: f32) -> Rot3 {
	let angle = rotation.angle();
	if angle > max {
		Rot3::identity().slerp(&rotation, max / angle)
	} else {
		rotation
	}
}

fn ease_in_out_cubic(t: f32) -> f32 {
	if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}
