use crate::application::Hand;
use crate::math::{Point3, Vec3};
use super::super::super::model::mmd::BodyPart;
use super::{Character, Ctx, EaseBlend};
use super::look::LookTarget;


/// Player must put both hands on her back, below the head, while facing her. Scaled down a bit from
/// OpenViva's 0.2 m because Miku is shorter than OpenViva's character.
const HUG_HAND_PADDING: f32 = 0.15;
/// Player head must be above the spine by at most this much.
const HUG_HEAD_MIN_HEIGHT: f32 = 0.45;
/// Player distance at which the hug animation leans to one side.
const HUG_SIDE_MIN_DISTANCE: f32 = 0.3;
const HUG_SIDE_MAX_DISTANCE: f32 = 0.3;

/// Standing hug (OpenViva's `HugListenerBehavior`): when the player wraps both arms around her she
/// switches to the hug body state, leans left or right depending on how the player is positioned,
/// and returns to standing when the player lets go.
pub struct Hug {
	active: bool,
	/// -1..1 blend of the hug animation's left/right lean.
	anim_side: f32,
	/// Fades the hug in, so the lean doesn't jump when the hug starts.
	start: EaseBlend,
}

impl Hug {
	pub fn new() -> Self {
		Hug {
			active: false,
			anim_side: 0.0,
			start: EaseBlend::new(0.0),
		}
	}

	/// True while the player is holding her in a hug.
	pub fn is_active(&self) -> bool {
		self.active
	}
}

/// OpenViva's `Tools.GetClampedRatio`; handles the default min == max settings as a step.
fn clamped_ratio(low: f32, high: f32, value: f32) -> f32 {
	if low > high {
		return 1.0 - ((value - low) / (high - low)).clamp(0.0, 1.0);
	}
	if high <= low {
		return if value < low { 1.0 } else { 0.0 };
	}
	((value - low) / (high - low)).clamp(0.0, 1.0)
}

impl Character {
	/// OpenViva's `IsHandWithinHugDistance`.
	fn hand_within_hug_distance(&self, hand: Point3, padding: f32) -> bool {
		hand.y < self.head_position().y - padding
	}

	/// OpenViva's `IsAttemptingToHug`: both hands are on her back, below the head, and the player is
	/// facing her from the opposite direction.
	fn is_attempting_to_hug(&self, ctx: &Ctx) -> bool {
		let Some(torso) = self.body(BodyPart::Torso) else { return false };
		let spine3 = Point3::from(torso.position.translation.vector);
		let player_head = Point3::from(ctx.player.head.translation.vector);

		let mut head_to_spine = spine3 - player_head;
		head_to_spine.y = 0.0;
		let head_to_spine_sq = head_to_spine.norm_squared();
		if head_to_spine_sq == 0.0 || player_head.y < spine3.y - HUG_HEAD_MIN_HEIGHT {
			return false;
		}

		let Some(right_hand) = ctx.player.hand(Hand::Right) else { return false };
		let Some(left_hand) = ctx.player.hand(Hand::Left) else { return false };
		let right = Point3::from(right_hand.position.translation.vector);
		let left = Point3::from(left_hand.position.translation.vector);
		if !self.hand_within_hug_distance(right, HUG_HAND_PADDING)
		|| !self.hand_within_hug_distance(left, HUG_HAND_PADDING) {
			return false;
		}

		let mut right_to_head = player_head - right;
		right_to_head.y = 0.0;
		let mut left_to_head = player_head - left;
		left_to_head.y = 0.0;

		let player_forward = ctx.player.head.rotation * -Vec3::z();
		let her_forward = self.forward();
		let opposite = (player_forward.x * her_forward.x + player_forward.z * her_forward.z) < 0.0;
		let sideways = Vec3::new(head_to_spine.z, 0.0, -head_to_spine.x);

		opposite
		&& right_to_head.norm_squared() > head_to_spine_sq
		&& left_to_head.norm_squared() > head_to_spine_sq
		&& sideways.dot(&right_to_head) != sideways.dot(&left_to_head)
	}

	fn begin_hug(&mut self, ctx: &Ctx) {
		self.log("Hug start");
		self.hug.active = true;
		self.hug.anim_side = 0.0;
		self.hug.start.reset(0.0);
		self.hug.start.start(1.0, 1.0);
		self.stop_move_to();
		self.set_target_animation(if self.is_happy() { "STAND_TO_STAND_HUG_FAR_HAPPY" } else { "STAND_TO_STAND_HUG_FAR_ANGRY" });
		self.set_look_at(LookTarget::PlayerHead);
		self.face_towards(Point3::from(ctx.player.head.translation.vector), 2.0);
	}

	fn stop_hug(&mut self) {
		if !self.hug.active { return; }
		self.log("Hug end");
		self.hug.active = false;
		self.hug.anim_side = 0.0;
		self.hug.start.reset(0.0);
		self.animator.set_param("hugSide", 0.0);
		self.set_target_animation("STAND_HUG_FAR_TO_STAND");
	}

	pub(super) fn stop_hug_silently(&mut self) {
		self.hug.active = false;
		self.hug.anim_side = 0.0;
		self.hug.start.reset(0.0);
		self.animator.set_param("hugSide", 0.0);
	}

	/// OpenViva's `FixedUpdateHuggingPlayer`.
	fn update_hugging(&mut self, ctx: &Ctx) {
		self.hug.start.update(ctx.dt);

		let player_head = Point3::from(ctx.player.head.translation.vector);
		let player_floor = ctx.player.floor_pos();
		let my_floor = self.floor_pos();

		let bearing = self.bearing(self.head_position(), self.forward(), player_head);
		let proximity = (player_floor - my_floor).xz().norm();
		let proximity_dist = 1.0 - clamped_ratio(HUG_SIDE_MIN_DISTANCE, HUG_SIDE_MAX_DISTANCE, proximity);
		let mut new_side = proximity_dist * bearing.signum();

		// Lock to the side when the player is really close.
		if new_side.abs() > 0.8 {
			new_side = new_side.signum();
			if self.is_happy() {
				self.speak_at_random_intervals("RELIEF", 3.0, 4.0, ctx);
			} else {
				self.speak_at_random_intervals("ANGRY_GRUMBLE_LONG", 3.0, 4.0, ctx);
			}
		}

		let blend = (ctx.dt * 8.0 * self.hug.start.value).min(1.0);
		self.hug.anim_side += (new_side - self.hug.anim_side) * blend;
		self.animator.set_param("hugSide", self.hug.anim_side);
		self.set_look_at(LookTarget::PlayerHead);
		self.look.set_awareness_timeout(0.5);
	}

	pub(super) fn update_hug(&mut self, ctx: &Ctx) {
		if !self.anim.has_balance {
			self.stop_hug_silently();
			return;
		}

		if self.hug.active {
			if self.is_attempting_to_hug(ctx) {
				self.update_hugging(ctx);
			} else {
				self.stop_hug();
			}
		} else if self.anim.body_state == "STANDING_HUG" {
			// Let go while the hug animation was still fading in.
			self.set_target_animation("STAND_HUG_FAR_TO_STAND");
		} else if self.anim.body_state == "STAND" && self.is_attempting_to_hug(ctx) {
			self.begin_hug(ctx);
		}
	}
}
