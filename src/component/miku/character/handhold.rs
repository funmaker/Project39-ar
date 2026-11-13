use crate::application::Hand;
use crate::math::Point3;
use super::super::super::model::mmd::BodyPart;
use super::{Character, Ctx};
use super::look::LookTarget;


/// Hand holding (OpenViva's `HandholdBehavior`): when the player holds her hand, she gets embarrassed
/// and follows when pulled.
pub struct Handhold {
	/// Which of her hands are held, `[left, right]`, with the player's hand holding it.
	held: [Option<Hand>; 2],
	pull_timer: f32,
}

impl Handhold {
	pub fn new() -> Self {
		Handhold {
			held: [None, None],
			pull_timer: 0.0,
		}
	}

	pub fn any_held(&self) -> bool {
		self.held.iter().any(Option::is_some)
	}
}

impl Character {
	pub(super) fn handhold_clear(&mut self) {
		self.handhold.held = [None, None];
	}

	fn add_handhold(&mut self, right: bool, source: Hand, ctx: &Ctx) {
		self.log(format!("Handhold {}", if right { "right" } else { "left" }));
		let side = if right { "RIGHT" } else { "LEFT" };
		if self.is_happy() {
			self.set_target_animation(&format!("STAND_HANDHOLD_HAPPY_EMBARRASSED_{side}"));
			self.speak("EMBARRASSED_HANDHOLD", false, ctx);
		} else {
			self.set_target_animation(&format!("STAND_HANDHOLD_ANGRY_REFUSE_{side}"));
		}
		self.handhold.held[right as usize] = Some(source);
		self.handhold.pull_timer = 0.0;
		self.stop_move_to();
	}

	fn remove_handhold(&mut self, right: bool) {
		if self.handhold.held[right as usize].take().is_none() { return; }
		self.log("Handhold end");
		if !self.handhold.any_held() && self.anim.has_balance {
			self.stop_move_to();
			self.override_clear_animation_priority();
			let idle = self.last_returnable_idle();
			self.set_target_animation(&idle);
		}
	}

	/// OpenViva's `HandholdBehavior.OnFixedUpdate`/`LateUpdateHandhold`: when the arm is stretched, she follows
	/// the player's hand.
	pub(super) fn update_handhold(&mut self, ctx: &Ctx) {
		for (right, part) in [(false, BodyPart::LeftHand), (true, BodyPart::RightHand)] {
			let grabbed = self.body(part).and_then(|b| b.grabbed);
			let held = self.handhold.held[right as usize];
			match (grabbed, held) {
				(Some(source), None) if self.anim.has_balance && !self.puppet.constrained => self.add_handhold(right, source, ctx),
				(None, Some(_)) => self.remove_handhold(right),
				_ => {},
			}
		}

		if !self.handhold.any_held() { return; }

		// Let the held arms follow the player's hand.
		for (right, parts) in [(false, [BodyPart::LeftHand, BodyPart::LeftForearm, BodyPart::LeftArm]),
		                       (true, [BodyPart::RightHand, BodyPart::RightForearm, BodyPart::RightArm])] {
			if self.handhold.held[right as usize].is_some() {
				for part in parts {
					self.puppet.part_pin.insert(part, 0.0);
					self.puppet.part_muscle.insert(part, 0.4);
				}
			}
		}

		for right in [false, true] {
			let Some(source) = self.handhold.held[right as usize] else { continue };
			let Some(hand) = ctx.player.hand(source) else { continue };
			let hand_pos = Point3::from(hand.position.translation.vector);
			let (hand_part, arm_part) = if right { (BodyPart::RightHand, BodyPart::RightArm) } else { (BodyPart::LeftHand, BodyPart::LeftArm) };
			let shoulder = self.body(arm_part).map(|b| Point3::from(b.position.translation.vector)).unwrap_or(hand_pos);
			let arm_length = self.body(hand_part).zip(self.body(arm_part))
			                     .map(|(h, a)| (h.rest.translation.vector - a.rest.translation.vector).norm())
			                     .unwrap_or(0.4);
			let stretch = (hand_pos - shoulder).norm();
			let speed = self.animator.param("speed");

			if stretch > arm_length * 1.25 || speed > 1.3 {
				// Pulled: follow the hand.
				self.handhold.pull_timer += ctx.dt;
				self.face_towards(hand_pos, 1.0);
				if self.time % 1.0 > 0.8 {
					self.set_look_at(LookTarget::PlayerHand(source));
					self.look.set_awareness_timeout(1.0);
				}
				if self.anim.body_state == "STAND" {
					self.move_to(hand_pos, arm_length * 0.9);
					if speed > 1.5 {
						let side = if right { "RIGHT" } else { "LEFT" };
						let anim = if self.tired { format!("STAND_TIRED_HANDHOLD_PULL_{side}") } else { format!("STAND_HANDHOLD_HAPPY_PULL_{side}") };
						self.set_target_animation(&anim);
					}
				} else if self.anim.body_state == "FLOOR_SIT" || self.anim.body_state == "SQUAT" {
					// Pulled up from sitting.
					if self.handhold.pull_timer > 0.7 {
						let anim = if self.anim.body_state == "SQUAT" { "SQUAT_TO_STAND" } else { "FLOOR_SIT_TO_STAND" };
						self.set_target_animation(anim);
					}
				}
			} else {
				self.handhold.pull_timer = 0.0;
				if self.puppet.is_move_active() && stretch < arm_length * 0.8 {
					self.stop_move_to();
				}
			}
		}
	}
}
