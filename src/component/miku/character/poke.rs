use crate::application::Hand;
use crate::math::{Point3, Vec3};
use super::{Character, Ctx, EaseBlend, PlayerHand};
use super::look::LookTarget;


/// Face and tummy pokes (OpenViva's `PokeBehavior`).
pub struct Poke {
	last_poke_time: f32,
	pub count: i32,
	pub post_face_poke: Option<String>,
	pub post_tummy_poke: Option<String>,
	new_face_poked: Option<String>,
	last_cheek_wipe_time: f32,
	last_block_time: f32,
	last_source: Option<Hand>,
	tummy_x: EaseBlend,
	laugh_timer: f32,
	laugh_look_timer: f32,
	/// Hand was outside of the face/tummy last frame, `[face, tummy][hand]`.
	outside: [[bool; 2]; 2],
}

impl Poke {
	pub fn new() -> Self {
		Poke {
			last_poke_time: f32::NEG_INFINITY,
			count: 0,
			post_face_poke: None,
			post_tummy_poke: None,
			new_face_poked: None,
			last_cheek_wipe_time: f32::NEG_INFINITY,
			last_block_time: f32::NEG_INFINITY,
			last_source: None,
			tummy_x: EaseBlend::new(0.0),
			laugh_timer: 0.0,
			laugh_look_timer: 1.0,
			outside: [[true; 2]; 2],
		}
	}
}

fn hand_slot(hand: Hand) -> usize {
	match hand { Hand::Left => 0, Hand::Right => 1 }
}

impl Character {
	/// Returns the player hands that just entered the sphere while moving towards it quickly enough.
	fn poking_hands(&mut self, area: usize, center: Point3, radius: f32, min_sq_speed: f32, ctx: &Ctx) -> Vec<PlayerHand> {
		let mut result = vec![];
		for hand in &ctx.player.hands {
			let pos = Point3::from(hand.position.translation.vector);
			let inside = (pos - center).norm() < radius;
			let slot = hand_slot(hand.hand);
			let was_outside = self.poke.outside[area][slot];
			self.poke.outside[area][slot] = !inside;
			if !inside || !was_outside || hand.holding { continue; }
			if (center - pos).dot(&hand.velocity) < 0.0 { continue; }
			if hand.velocity.norm_squared() < min_sq_speed { continue; }
			result.push(hand.clone());
		}
		result
	}

	pub(super) fn update_poke(&mut self, ctx: &Ctx) {
		let face = self.sphere_position(&self.config.face, ctx.app);
		for hand in self.poking_hands(0, face, self.config.face.radius, 0.002, ctx) {
			if self.headpat.source == Some(hand.hand) { continue; }
			self.attempt_face_poke(&hand, ctx);
		}

		let tummy = self.sphere_position(&self.config.tummy, ctx.app);
		for hand in self.poking_hands(1, tummy, self.config.tummy.radius, 0.04, ctx) {
			self.attempt_tummy_poke(&hand, ctx);
		}

		// Laughing from tummy pokes.
		if self.anim.current == "STAND_POKED_TUMMY_LOOP" {
			self.poke.tummy_x.update(ctx.dt);
			self.animator.set_param("pokeTummyX", self.poke.tummy_x.value);
			if self.poke.laugh_timer <= 0.0 {
				self.poke.laugh_timer = 0.0;
				self.set_target_animation("STAND_POKED_TUMMY_OUT");
			} else {
				self.poke.laugh_timer -= ctx.dt;
			}
			self.poke.laugh_look_timer -= ctx.dt;
			if self.poke.laugh_look_timer < 0.0 {
				self.poke.laugh_look_timer = 0.4 + Self::random() * 0.5;
				self.set_look_at(LookTarget::PlayerHead);
				let flags = if Self::random() > 0.5 { 3 } else { 0 };
				self.look.set_body_variables(flags, 1.0, self.poke.laugh_look_timer * 0.7);
			}
		}
	}

	/// OpenViva's `AttemptFacePoke`.
	fn attempt_face_poke(&mut self, hand: &PlayerHand, ctx: &Ctx) -> bool {
		if !self.anim.has_balance || self.is_animation_changing_body_state() { return false; }
		if self.poke.last_source == Some(hand.hand) && self.time - self.poke.last_poke_time < 0.5 { return false; }

		self.poke.last_source = Some(hand.hand);
		let new_count = (self.poke.count - ((self.time - self.poke.last_poke_time) / 4.0) as i32).clamp(0, 6);
		let pos = Point3::from(hand.position.translation.vector);
		let bearing = self.bearing(self.head_position(), self.forward(), pos);
		let side = (bearing < 0.0) as usize;

		let mut anim = None;
		if self.poke.count > 4 {
			self.face_towards(pos, 2.0);
			if Self::random() < 0.4 && self.time - self.poke.last_block_time > 2.0 && !self.tired {
				anim = Some(self.block_face_poke_animation(side));
			}
			if anim.is_none() {
				anim = self.face_poked_animation(side);
			} else {
				self.shift_happiness(-2);
				let mut push = self.head_position() - pos;
				push.y = 0.0;
				let push = push.try_normalize(0.0001).unwrap_or(Vec3::zeros());
				self.play_force(push * (0.2 + Self::random() * 0.8), 0.2 + Self::random() * 0.3);
			}
		} else {
			anim = self.face_poked_animation(side);
		}

		let Some(anim) = anim else { return false };
		self.log(format!("Face poke ({anim})"));
		self.poke.count = new_count;
		self.poke.new_face_poked = Some(anim.clone());
		self.set_target_animation(&anim);
		self.poke.post_face_poke = Some(self.post_face_poke_animation(side).unwrap_or_else(|| self.last_returnable_idle()));
		let _ = ctx;
		true
	}

	fn block_face_poke_animation(&self, side: usize) -> String {
		let base = if Self::random() > 0.5 { "STAND_HEADPAT_CLIMAX_TO_CANCEL_RIGHT" } else { "STAND_ANGRY_BLOCK_RIGHT" };
		self.data.offset(base, side).unwrap_or(base).to_string()
	}

	fn face_poked_animation(&self, side: usize) -> Option<String> {
		match self.anim.body_state.as_str() {
			"STAND" if self.tired => Some(self.animation_variant("STAND_TIRED_POKE_RIGHT", side, 1)),
			"STAND" => Some(self.animation_variant("STAND_POKE_FACE_1_RIGHT", side, 3)),
			"SQUAT" => Some(self.animation_variant("SQUAT_FACE_POKE_1_RIGHT", side, 2)),
			_ => None,
		}
	}

	fn post_face_poke_animation(&self, side: usize) -> Option<String> {
		if self.anim.body_state != "STAND" { return None; }
		if self.tired { return Some(self.last_returnable_idle()); }
		if self.poke.count >= 2 && self.time - self.poke.last_cheek_wipe_time > 15.0 {
			return Some(if side == 0 { "STAND_WIPE_CHEEK_RIGHT" } else { "STAND_WIPE_CHEEK_LEFT" }.into());
		}
		None
	}

	/// OpenViva's `AttemptTummyPoke`.
	fn attempt_tummy_poke(&mut self, hand: &PlayerHand, ctx: &Ctx) -> bool {
		if self.anim.body_state != "STAND" || !self.anim.has_balance { return false; }
		self.poke.last_source = Some(hand.hand);
		let pos = Point3::from(hand.position.translation.vector);
		self.log("Tummy poke");

		match self.anim.current.as_str() {
			"STAND_POKED_TUMMY_LOOP" => self.update_tummy_poke_x(pos, ctx),
			"STAND_POKED_TUMMY_IN" => {
				self.poke.post_tummy_poke = Some("STAND_POKED_TUMMY_LOOP".into());
				self.update_tummy_poke_x(pos, ctx);
			},
			"STAND_POKED_TUMMY_OUT" => {
				if self.torso_time() < 0.6 {
					self.override_clear_animation_priority();
					self.set_target_animation("STAND_POKED_TUMMY_LOOP");
					self.update_tummy_poke_x(pos, ctx);
				} else {
					self.set_target_animation("STAND_POKED_TUMMY_IN");
					self.speak("STARTLE_SHORT", false, ctx);
				}
			},
			_ => {
				self.set_target_animation("STAND_POKED_TUMMY_IN");
				self.poke.post_tummy_poke = Some("STAND_POKED_TUMMY_OUT".into());
				self.speak("STARTLE_SHORT", false, ctx);
			},
		}

		let player = ctx.player.floor_pos();
		self.face_towards(Point3::from((player.coords + pos.coords) / 2.0), 1.0);
		true
	}

	fn update_tummy_poke_x(&mut self, source: Point3, ctx: &Ctx) {
		let chest = self.body(crate::component::model::mmd::BodyPart::Torso).map(|b| Point3::from(b.position.translation.vector)).unwrap_or(self.floor_pos());
		let poke_x = self.bearing(chest, self.forward(), source);
		self.poke.tummy_x.start((poke_x / 50.0).round().clamp(-1.0, 1.0), 0.6);

		if self.is_speaking("LAUGH_LONG") { return; }
		if Self::random() > 0.85 {
			self.poke.laugh_timer = 1.3;
			self.speak("LAUGH_LONG", false, ctx);
		} else {
			self.poke.laugh_timer = 0.45;
			self.speak("LAUGH_SHORT", false, ctx);
		}
	}

	pub(super) fn poke_on_animation_change(&mut self, _old: &str, new: &str) {
		if self.poke.new_face_poked.as_deref() == Some(new) {
			self.poke.count += 1;
			self.poke.last_poke_time = self.time;
			self.look_at_last_poke_source();
		}
		match new {
			"STAND_WIPE_CHEEK_RIGHT" | "STAND_WIPE_CHEEK_LEFT" | "STAND_HEADPAT_CLIMAX_TO_CANCEL_RIGHT" => {
				self.poke.last_cheek_wipe_time = self.time;
				self.set_look_at(LookTarget::PlayerHead);
				self.look.set_awareness_timeout(0.5);
			},
			"STAND_ANGRY_BLOCK_RIGHT" | "STAND_ANGRY_BLOCK_LEFT" => {
				self.poke.last_block_time = self.time;
				self.poke.last_poke_time = self.time;
				self.look_at_last_poke_source();
			},
			"STAND_POKED_TUMMY_IN" => self.poke.tummy_x.reset(0.0),
			_ => {},
		}
		if _old == "STAND_POKED_TUMMY_LOOP" {
			self.poke.tummy_x.reset(0.0);
		}
	}

	fn look_at_last_poke_source(&mut self) {
		if let Some(source) = self.poke.last_source {
			self.set_look_at(LookTarget::PlayerHand(source));
			self.look.set_awareness_timeout(0.5);
		}
	}
}
