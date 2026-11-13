use crate::math::{Isometry3, Point3, Vec3};
use super::super::anim::human;
use super::{Character, Ctx};
use super::look::LookTarget;


/// Cheek position in head space, from OpenViva's `KissingBehavior`.
const CHEEK_OFFSET: Vec3 = Vec3::new(0.019, 0.016, 0.018);
/// Player head distance at which a cheek kiss starts.
const KISS_DISTANCE: f32 = 0.22;

/// Cheek kissing (OpenViva's `KissingBehavior`): the player's head touching one of her cheeks makes
/// her turn her head and react. She is happy about it, or annoyed when she is angry.
pub struct Kiss {
	/// Animation to play after an angry cheek kiss.
	pub post_kiss_anim: Option<String>,
	/// A kiss animation is playing; don't start another one.
	anim_busy: bool,
}

impl Kiss {
	pub fn new() -> Self {
		Kiss {
			post_kiss_anim: None,
			anim_busy: false,
		}
	}
}

impl Character {
	/// World transform of the animated head bone.
	pub(super) fn animated_head(&self) -> Isometry3 {
		let bone = self.retarget.mmd_bone(human::HEAD).unwrap_or(0);
		self.puppet.anchor() * self.bone_world.get(bone).copied().unwrap_or_else(Isometry3::identity)
	}

	/// Returns the sign of the cheek the player's head is close to: -1 left, 1 right.
	fn near_cheek(&self, ctx: &Ctx) -> Option<f32> {
		let head = self.animated_head();
		let player_head = Point3::from(ctx.player.head.translation.vector);

		for side in [-1.0, 1.0] {
			let offset = Vec3::new(CHEEK_OFFSET.x.abs() * side, CHEEK_OFFSET.y, CHEEK_OFFSET.z);
			let cheek = head * Point3::from(offset);
			if (player_head - cheek).norm() < KISS_DISTANCE {
				return Some(side);
			}
		}
		None
	}

	/// OpenViva's `checkKissingProximity`.
	pub(super) fn update_kiss(&mut self, ctx: &Ctx) {
		// Kissing is disabled while hugging. Don't request the same kiss every frame while an
		// animation is still cross-fading in (the request is dropped if a higher priority
		// animation is playing).
		if self.hug.is_active() || self.kiss.anim_busy {
			return;
		}
		if self.anim.current.starts_with("STAND_KISS_") || self.anim.target.starts_with("STAND_KISS_") {
			return;
		}
		if self.anim.body_state != "STAND" || !self.anim.has_balance {
			return;
		}

		let Some(side) = self.near_cheek(ctx) else { return };
		let right = side > 0.0;

		let (anim, post) = if self.is_happy() {
			(format!("STAND_KISS_HAPPY_CHEEK_{}", if right { "RIGHT" } else { "LEFT" }), None)
		} else {
			let post = if Self::random() > 0.95 {
				format!("STAND_KISS_ANGRY_{}_TO_ANGRY", if right { "LEFT" } else { "RIGHT" })
			} else {
				format!("STAND_KISS_ANGRY_{}_TO_HAPPY", if right { "LEFT" } else { "RIGHT" })
			};
			(format!("STAND_KISS_ANGRY_CHEEK_{}", if right { "RIGHT" } else { "LEFT" }), Some(post))
		};

		// A higher priority animation (eg. the proximity startle) is playing; try again once it is over.
		if self.info(&anim).map_or(true, |info| self.anim.highest_target_priority > info.priority) {
			return;
		}

		self.log(format!("Kiss ({anim})"));
		self.kiss.post_kiss_anim = post;
		self.set_target_animation(&anim);
		self.set_look_at(LookTarget::PlayerHead);
		self.face_towards(Point3::from(ctx.player.head.translation.vector), 2.0);
	}

	pub(super) fn kiss_on_animation_change(&mut self, old: &str, new: &str) {
		if new.starts_with("STAND_KISS_") {
			self.kiss.anim_busy = true;
		}
		if old.starts_with("STAND_KISS_") {
			self.kiss.anim_busy = false;
		}
		if matches!(new, "STAND_KISS_ANGRY_LEFT_TO_HAPPY" | "STAND_KISS_ANGRY_RIGHT_TO_HAPPY") {
			self.shift_happiness(2);
		}
	}
}
