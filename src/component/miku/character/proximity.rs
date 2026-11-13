use crate::math::{Point3, Vec3};
use super::{Character, Ctx};
use super::look::LookTarget;


/// Reaction to the player's face getting close (OpenViva's `ProximityBehavior`).
pub struct Proximity {
	start_timer: f32,
	hang_timer: f32,
	last_local: Option<Vec3>,
	last_distance: f32,
	look_timer: f32,
	active: bool,
}

impl Proximity {
	pub fn new() -> Self {
		Proximity {
			start_timer: 0.0,
			hang_timer: 0.0,
			last_local: None,
			last_distance: 0.0,
			look_timer: 0.0,
			active: false,
		}
	}
}

impl Character {
	pub(super) fn update_proximity(&mut self, ctx: &Ctx) {
		if self.headpat.is_active() || self.anim.body_state != "STAND" || !self.anim.has_balance {
			self.end_proximity(false);
			return;
		}
		if self.look.target != LookTarget::PlayerHead {
			self.end_proximity(true);
			return;
		}

		let player_head = Point3::from(ctx.player.head.translation.vector);
		let head = self.head_position();
		let distance = (player_head - head).norm();
		let anchor_inv = self.puppet.anchor().inverse();
		let local = (anchor_inv * player_head).coords;
		let last_local = *self.proximity.last_local.get_or_insert(local);
		let in_front = (player_head - head).dot(&self.forward()) > -0.2;

		if in_front && distance < 0.5 {
			if distance - self.proximity.last_distance < 0.004 && (local - last_local).norm_squared() > 0.01 {
				self.start_proximity_fast(player_head);
			} else if self.is_current_animation_idle() && !self.proximity.active {
				self.proximity.start_timer += ctx.dt;
				if self.proximity.start_timer > 0.5 {
					self.start_proximity_slow(player_head);
				}
			}
			self.proximity.hang_timer = 0.6;
		} else if self.proximity.active {
			self.proximity.hang_timer -= ctx.dt;
			if self.proximity.hang_timer <= 0.0 {
				self.end_proximity(true);
			}
		} else {
			self.proximity.start_timer = 0.0;
		}

		self.proximity.last_distance = distance;
		self.proximity.last_local = Some(local);

		if self.anim.current == "STAND_FACE_PROX_HAPPY_LOOP" {
			self.proximity.look_timer -= ctx.dt;
			if self.proximity.look_timer < 0.0 {
				self.proximity.look_timer = 10.0;
				self.set_look_at(LookTarget::PlayerHead);
			}
		}
	}

	fn end_proximity(&mut self, return_to_idle: bool) {
		let was_active = self.proximity.active;
		self.proximity.active = false;
		self.proximity.last_local = None;
		if return_to_idle && was_active && matches!(self.anim.current.as_str(), "STAND_FACE_PROX_HAPPY_LOOP" | "STAND_FACE_PROX_ANGRY_LOOP") {
			let idle = self.last_returnable_idle();
			self.set_target_animation(&idle);
		}
	}

	fn start_proximity_fast(&mut self, player_head: Point3) {
		if self.proximity.active { return; }
		self.log("Proximity: startled");
		self.proximity.active = true;
		self.set_target_animation(if self.is_happy() { "STAND_FACE_PROX_HAPPY_SURPRISE" } else { "STAND_FACE_PROX_ANGRY_SURPRISE" });
		self.face_towards(player_head, 2.0);
		let mut push = self.floor_pos() - player_head;
		push.y = 0.0;
		if let Some(push) = push.try_normalize(0.0001) {
			self.play_force(push * 1.5, 0.3);
		}
	}

	fn start_proximity_slow(&mut self, player_head: Point3) {
		self.log("Proximity: close");
		self.proximity.start_timer = 0.0;
		self.proximity.active = true;
		self.set_target_animation(if self.is_happy() { "STAND_FACE_PROX_HAPPY_LOOP" } else { "STAND_FACE_PROX_ANGRY_LOOP" });
		self.face_towards(player_head, 1.0);
	}

	pub(super) fn proximity_on_animation_change(&mut self, _old: &str, new: &str) {
		if matches!(new, "STAND_FACE_PROX_ANGRY_SURPRISE" | "STAND_FACE_PROX_HAPPY_SURPRISE") {
			self.look.set_awareness_timeout(2.0);
		}
	}
}
