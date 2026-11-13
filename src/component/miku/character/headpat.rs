use crate::application::Hand;
use crate::math::{Point3, Vec3};
use super::super::super::physics::collider::ColliderComponent;
use super::{Character, Ctx, EaseBlend};
use super::look::LookTarget;


/// Headpat state (OpenViva's `HeadpatBehavior`). Gentle pats ("proper") make her happy, rough ones
/// annoy her and eventually end the headpat.
pub struct Headpat {
	pub source: Option<Hand>,
	blend: EaseBlend,
	wants_more: bool,
	roughness_tolerance: f32,
	proper: f32,
	proper_cooldown: f32,
	roughness: f32,
	roughness_cooldown: f32,
	proper_total: f32,
	force: Vec3,
	last_idle_sad_time: f32,
	accept_after: f32,
	refuse_going_happy: bool,
	/// The hand was outside of the head last frame.
	hand_was_outside: [bool; 2],
	/// Player hand whose colliders were made non-solid, so it doesn't push her head down while patting.
	ghost_hand: Option<Hand>,
}

impl Headpat {
	pub fn new() -> Self {
		Headpat {
			source: None,
			blend: EaseBlend::new(0.0),
			wants_more: false,
			roughness_tolerance: 0.0,
			proper: 0.0,
			proper_cooldown: 0.0,
			roughness: 0.0,
			roughness_cooldown: 0.0,
			proper_total: 0.0,
			force: Vec3::zeros(),
			last_idle_sad_time: f32::NEG_INFINITY,
			accept_after: 0.0,
			refuse_going_happy: false,
			hand_was_outside: [true, true],
			ghost_hand: None,
		}
	}

	pub fn is_active(&self) -> bool {
		self.blend.target() == 1.0
	}
}

fn hand_slot(hand: Hand) -> usize {
	match hand { Hand::Left => 0, Hand::Right => 1 }
}

impl Character {
	/// Makes a player hand's colliders sensors (or solid again).
	fn set_hand_ghost(&self, hand: Hand, ghost: bool, ctx: &Ctx) {
		let Some(entity) = ctx.player.hand(hand).and_then(|h| h.entity.get(ctx.app)) else { return };
		let mut physics = ctx.app.physics.borrow_mut();
		for collider in entity.iter_component_by_type::<ColliderComponent>() {
			if let Some(collider) = physics.colliders.get_mut(collider.handle()) {
				collider.set_sensor(ghost);
			}
		}
	}
	
	pub fn is_headpat_active(&self) -> bool {
		self.headpat.is_active()
	}

	fn headpat_animation(&self, set: &str) -> Option<String> {
		self.animation_from_set(set)
	}

	fn headpat_start_animation(&self) -> Option<String> {
		if self.tired {
			self.headpat_animation("HEADPAT_START_TIRED")
		} else if self.is_happy() {
			if self.anim.body_state == "STAND" && self.headpat.wants_more {
				Some("STAND_HEADPAT_HAPPY_AFTER_MORE".into())
			} else {
				self.headpat_animation("HEADPAT_START_HAPPY")
			}
		} else {
			self.headpat_animation("HEADPAT_START_ANGRY")
		}
	}

	fn headpat_idle_animation(&self) -> Option<String> {
		match self.anim.body_state.as_str() {
			"STAND" => Some(if self.tired { "STAND_TIRED_HEADPAT_IDLE".into() } else { self.animation_from_mood("STAND_HEADPAT_HAPPY_LOOP") }),
			"FLOOR_SIT" => Some(self.animation_from_mood("FLOOR_SIT_LOCOMOTION_HAPPY")),
			"SQUAT" => Some(self.animation_from_mood("SQUAT_HEADPAT_HAPPY_LOOP")),
			_ => None,
		}
	}

	/// OpenViva's `GetRegularHeadpatEndAnim`.
	fn regular_headpat_end_animation(&mut self) -> Option<String> {
		match self.anim.body_state.as_str() {
			"STAND" => {
				if self.is_happy() {
					self.headpat.wants_more = !self.headpat.wants_more;
					if !self.headpat.wants_more {
						if Self::random() > 0.5 && self.headpat.proper_total > 2.0 {
							Some("STAND_HEADPAT_HAPPY_SATISFACTION".into())
						} else {
							Some("STAND_HAPPY_IDLE1".into())
						}
					} else {
						Some("STAND_HEADPAT_HAPPY_WANTED_MORE".into())
					}
				} else {
					Some(match (Self::random() * 3.0) as i32 {
						0 => "STAND_ANGRY_IDLE1",
						1 => "STAND_HEADPAT_INTERRUPT",
						_ => "STAND_HEADPAT_ANGRY_END",
					}.into())
				}
			},
			"FLOOR_SIT" | "SQUAT" => {
				if self.is_happy() {
					self.headpat.wants_more = !self.headpat.wants_more;
					if self.headpat.wants_more {
						return Some(if self.anim.body_state == "SQUAT" { "SQUAT_HEADPAT_HAPPY_WANTED_MORE" } else { "FLOOR_SIT_HEADPAT_HAPPY_WANTED_MORE" }.into());
					}
				}
				None
			},
			_ => None,
		}
	}

	/// Detects a player hand landing on top of the head (OpenViva's `AttemptBeginHeadpat`).
	fn attempt_begin_headpat(&mut self, ctx: &Ctx) -> bool {
		if self.headpat.is_active() || self.time < self.headpat.accept_after { return false; }
		if !self.anim.has_balance || self.is_animation_changing_body_state() { return false; }
		if !self.active_allows_headpat() { return false; }

		let center = self.sphere_position(&self.config.headpat, ctx.app);
		let radius = self.config.headpat.radius;
		let head = self.head_position();

		let mut source = None;
		for hand in &ctx.player.hands {
			let pos = Point3::from(hand.position.translation.vector);
			let inside = (pos - center).norm() < radius + 0.02;
			let slot = hand_slot(hand.hand);
			let was_outside = self.headpat.hand_was_outside[slot];
			self.headpat.hand_was_outside[slot] = !inside;
			if !inside || !was_outside || hand.holding { continue; }
			// Must come from above, not moving away from the head.
			if hand.velocity.dot(&(pos - head)) > 0.0 { continue; }
			if pos.y - head.y < 0.08 { continue; }
			source = Some(hand.hand);
		}

		let Some(source) = source else { return false };
		let Some(start) = self.headpat_start_animation() else { return false };

		self.log(format!("Headpat start ({start})"));
		self.headpat.source = Some(source);
		self.headpat.blend.start(1.0, 0.8);
		self.headpat.roughness_tolerance = 0.5;
		self.headpat.proper = 0.0;
		self.headpat.proper_cooldown = 0.0;
		self.headpat.roughness = 0.0;
		self.headpat.roughness_cooldown = 0.0;
		self.headpat.proper_total = 0.0;
		self.override_clear_animation_priority();
		self.set_target_animation(&start);
		self.look.set_awareness_timeout(0.5);
		true
	}

	fn stop_headpat(&mut self, end_animation: Option<String>, accept_timeout: f32) {
		if self.headpat.source.is_none() { return; }
		self.log("Headpat end");
		self.headpat.accept_after = self.time + accept_timeout;
		self.headpat.blend.start(0.0, 0.4);
		self.override_clear_animation_priority();
		let anim = end_animation.unwrap_or_else(|| self.last_returnable_idle());
		self.set_target_animation(&anim);
		self.headpat.source = None;
		self.look.lean = None;
	}

	pub(super) fn stop_headpat_silently(&mut self) {
		self.headpat.source = None;
		self.headpat.blend.reset(0.0);
		self.look.lean = None;
	}

	fn end_headpat_as_rough(&mut self) {
		self.log(format!("Headpat too rough ({:.2} m/s)", self.headpat.force.norm()));
		self.headpat.wants_more = false;
		self.shift_happiness(-2);
		let anim = self.animation_from_set("HEADPAT_BRUSH_AWAY");
		self.stop_headpat(anim, 1.0);
	}

	fn succeed_in_proper_headpat(&mut self) {
		if !self.is_happy() {
			self.headpat.proper_total = 0.0;
			if self.headpat.refuse_going_happy {
				let anim = self.animation_from_set("HEADPAT_CANCEL_SUCCESS");
				self.stop_headpat(anim, 1.0);
			} else {
				match self.animation_from_set("HEADPAT_SUCCESS") {
					Some(anim) => self.set_target_animation(&anim),
					None => {
						let idle = self.last_returnable_idle();
						self.set_target_animation(&idle);
					},
				}
				self.shift_happiness(4);
			}
			self.headpat.refuse_going_happy = !self.headpat.refuse_going_happy;
		}
	}

	pub(super) fn play_proper_headpat_sound(&mut self, ctx: &Ctx) {
		if self.headpat.proper_cooldown < 0.4 && self.headpat.roughness_cooldown == 1.0 {
			self.speak("HEADPAT_HAPPY", false, ctx);
		}
	}

	/// OpenViva's `RespondToHeadpatForces`.
	fn respond_to_headpat_forces(&mut self, ctx: &Ctx) {
		let delta = ctx.dt * self.anim.speed;
		if self.headpat.blend.value == 0.0 || delta == 0.0 || self.headpat.source.is_none() { return; }

		self.headpat.proper_cooldown = (self.headpat.proper_cooldown + delta).clamp(0.0, 1.0);
		self.headpat.roughness_cooldown = (self.headpat.roughness_cooldown + delta * 1.333).clamp(0.0, 1.0);

		let force = self.headpat.force.norm() / 20.0;
		if force > 0.09 {
			// Rough
			self.headpat.roughness += force;
			self.headpat.roughness_cooldown = 0.0;
			self.headpat.proper_cooldown = 1.0;
			self.headpat.roughness_tolerance -= delta;
			if self.headpat.roughness_tolerance < 0.0 {
				self.end_headpat_as_rough();
			}
			if let Some(source) = self.headpat.source {
				self.set_look_at(LookTarget::PlayerHand(source));
				self.look.set_awareness_timeout(1.0);
			}
		} else if force > 0.01 {
			// Proper
			self.headpat.proper = (self.headpat.proper + delta * 2.0).min(1.1);
			self.headpat.proper_cooldown = 0.0;
			if self.headpat.proper >= 1.0 {
				self.headpat.proper_total += delta;
				if self.headpat.proper_total > 2.0 {
					self.succeed_in_proper_headpat();
				}
			}
		} else {
			self.headpat.proper = (self.headpat.proper - delta * 4.0 * self.headpat.proper_cooldown).max(0.0);
		}

		if self.headpat.proper == 0.0 && self.headpat.roughness == 0.0 && self.headpat.proper_total > 0.0
		&& self.time - self.headpat.last_idle_sad_time > 7.0 {
			self.headpat.last_idle_sad_time = self.time;
			if self.is_happy() {
				let side = if Self::random() > 0.5 { "RIGHT" } else { "LEFT" };
				self.set_target_animation(&format!("STAND_HEADPAT_HAPPY_IDLE_SAD_{side}"));
				if let Some(source) = self.headpat.source {
					self.set_look_at(LookTarget::PlayerHand(source));
					self.look.set_awareness_timeout(1.0);
				}
			}
		}

		self.headpat.roughness = (self.headpat.roughness - delta * self.headpat.roughness_cooldown).clamp(0.0, 1.0);
		let applied_roughness = 1.0 - (1.0 - self.headpat.roughness).powi(2);
		let proper = self.headpat.proper.clamp(0.0, 1.0);
		let proper = if proper < 0.5 { 2.0 * proper * proper } else { 1.0 - (-2.0 * proper + 2.0).powi(2) / 2.0 };
		self.animator.set_param("HeadpatProper", proper);
		self.animator.set_param("HeadpatRoughness", applied_roughness);
	}

	/// OpenViva's `HeadpatBehavior.OnFixedUpdate` and the source hand tracking.
	pub(super) fn update_headpat(&mut self, ctx: &Ctx) {
		// In OpenViva the start animation is applied before the headpat logic runs again.
		let started = self.attempt_begin_headpat(ctx);
		
		if self.headpat.ghost_hand != self.headpat.source {
			if let Some(hand) = self.headpat.ghost_hand {
				self.set_hand_ghost(hand, false, ctx);
			}
			if let Some(hand) = self.headpat.source {
				self.set_hand_ghost(hand, true, ctx);
			}
			self.headpat.ghost_hand = self.headpat.source;
		}

		self.headpat.blend.update(ctx.dt);
		if self.headpat.blend.value == 0.0 {
			self.headpat.source = None;
			return;
		}

		// Track the patting hand.
		if let Some(source) = self.headpat.source {
			match ctx.player.hand(source) {
				Some(hand) => {
					// OpenViva uses the patting hand's velocity (smoothed when reading the player's hands).
					let pos = Point3::from(hand.position.translation.vector);
					let mut force = hand.velocity * self.headpat.blend.value;
					force.y = 0.0;
					self.headpat.force = force;
					let center = self.sphere_position(&self.config.headpat, ctx.app);
					let head = self.head_position();
					// Like OpenViva: head radius + 0.01, plus a margin.
					let away = (pos - center).norm() > self.config.headpat.radius + 0.01 + 0.085;
					if (away || pos.y < head.y) && self.headpat.blend.finished() {
						self.log(format!("Headpat hand left: dist {:.3} hand y {:.2} head y {:.2}", (pos - center).norm(), pos.y, head.y));
						let anim = self.regular_headpat_end_animation();
						self.stop_headpat(anim, 0.0);
						return;
					}
					let roughness = self.headpat.roughness;
					self.look.lean = Some((pos, self.headpat.blend.value * (0.3 + roughness * 0.4)));
				},
				None => {
					let anim = self.regular_headpat_end_animation();
					self.stop_headpat(anim, 0.0);
					return;
				},
			}
		}

		self.respond_to_headpat_forces(ctx);

		if self.headpat.is_active() && !started && let Some(idle) = self.headpat_idle_animation() {
			if self.headpat.roughness > 0.5 {
				if !self.is_speaking("SCREAMING") {
					self.speak("SCREAMING", true, ctx);
				}
				if self.anim.current != idle {
					self.override_clear_animation_priority();
					self.set_target_animation(&idle);
				}
			} else {
				if self.is_speaking("SCREAMING") {
					self.stop_speaking();
				}
				if self.is_current_animation_idle() && self.anim.current != idle {
					self.override_clear_animation_priority();
					self.set_target_animation(&idle);
				}
			}
		}
	}

	pub(super) fn headpat_on_animation_change(&mut self, old: &str, new: &str) {
		match new {
			"STAND_HEADPAT_HAPPY_WANTED_MORE" | "STAND_HEADPAT_ANGRY_LOOP" => {
				if let Some(source) = self.headpat.source {
					self.set_look_at(LookTarget::PlayerHand(source));
				}
				self.look.set_awareness_timeout(2.0);
				return;
			},
			_ => {},
		}
		if matches!(old, "STAND_HEADPAT_HAPPY_WANTED_MORE" | "STAND_HEADPAT_ANGRY_LOOP") {
			self.look.set_awareness_timeout(0.0);
		}
	}
}
