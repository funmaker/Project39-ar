use crate::application::Hand;
use crate::math::Point3;
use super::{Character, Ctx};
use super::gestures::Gesture;
use super::look::LookTarget;


/// Current active behavior (OpenViva's `ActiveBehaviors`).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ActiveTask {
	Idle,
	/// Following the player around.
	Follow,
	/// Playing the chopsticks finger game.
	Chopsticks,
}

impl ActiveTask {
	pub fn name(&self) -> &'static str {
		match self {
			ActiveTask::Idle => "Idle",
			ActiveTask::Follow => "Follow",
			ActiveTask::Chopsticks => "Chopsticks",
		}
	}
}

/// Idle behavior state (OpenViva's `IdleBehavior`).
pub struct Idle {
	next_variation_time: f32,
	idle_version: u32,
	face_target_timer: f32,
	/// Time the player has been sitting in front of her, to start the chopsticks game.
	chopsticks_invite_timer: f32,
}

impl Idle {
	pub fn new() -> Self {
		Idle {
			next_variation_time: 3.0,
			idle_version: 0,
			face_target_timer: 0.0,
			chopsticks_invite_timer: 0.0,
		}
	}
}

impl Character {
	pub fn set_task(&mut self, task: ActiveTask) {
		if self.active == task { return; }
		self.log(format!("Task {} -> {}", self.active.name(), task.name()));
		match self.active {
			ActiveTask::Follow => self.stop_move_to(),
			ActiveTask::Chopsticks => self.chopsticks_deactivate(),
			ActiveTask::Idle => {},
		}
		self.active = task;
	}

	/// OpenViva's `ActiveTask.RequestPermission(BEGIN_HEADPAT)`.
	pub(super) fn active_allows_headpat(&self) -> bool {
		self.active != ActiveTask::Chopsticks || !self.chopsticks.is_active()
	}

	pub fn play_available_refuse_animation(&mut self) {
		self.set_target_animation(if self.tired { "STAND_TIRED_REFUSE" } else { "STAND_REFUSE" });
	}

	fn available_wave_animation(&self) -> Option<String> {
		if self.anim.body_state != "STAND" { return None; }
		if self.is_happy() {
			let right_free = self.handhold.any_held() == false || self.body(crate::component::model::mmd::BodyPart::RightHand).map_or(true, |b| b.grabbed.is_none());
			Some(if right_free { "STAND_WAVE_HAPPY_RIGHT" } else { "STAND_WAVE_HAPPY_LEFT" }.into())
		} else {
			Some(if Self::random() > 0.5 { "STAND_HEADPAT_INTERRUPT" } else { "STAND_HEADPAT_ANGRY_END" }.into())
		}
	}

	/// Dispatches a player gesture to the behaviors (OpenViva's `ActiveBehaviors.OnGesture`).
	pub(super) fn on_gesture(&mut self, hand: Hand, gesture: Gesture, ctx: &Ctx) {
		let hand_pos = ctx.player.hand(hand).map(|h| Point3::from(h.position.translation.vector))
		                  .unwrap_or_else(|| Point3::from(ctx.player.head.translation.vector));
		match gesture {
			Gesture::Hello => {
				if self.is_current_animation_idle() && self.can_see_point(hand_pos) {
					if let Some(wave) = self.available_wave_animation() {
						self.set_target_animation(&wave);
						self.set_look_at(LookTarget::PlayerHand(hand));
						self.look.set_awareness_timeout(1.5);
						self.face_towards(hand_pos, 2.0);
					}
				}
			},
			Gesture::Follow => {
				if self.active == ActiveTask::Follow {
					// Beckoning again while following makes her stay.
					self.set_task(ActiveTask::Idle);
					self.set_target_animation("STAND_AGREE");
				} else {
					self.attempt_follow(ctx);
				}
			},
			Gesture::PresentStart | Gesture::PresentEnd => {},
		}
	}

	/// OpenViva's `FollowBehavior.AttemptFollow`.
	fn attempt_follow(&mut self, ctx: &Ctx) {
		let head = Point3::from(ctx.player.head.translation.vector);
		if !self.can_see_point(head) { return; }
		if self.is_happy() || self.tired {
			self.set_task(ActiveTask::Follow);
			self.speak("CONFIDENT", false, ctx);
		} else {
			self.play_available_refuse_animation();
			if self.anim.body_state == "STAND" {
				self.face_towards(head, 2.0);
			}
		}
	}

	/// Starts the chopsticks game (OpenViva's `ChopsticksBehavior.AttemptChopsticks`).
	pub fn attempt_chopsticks(&mut self) -> bool {
		if self.anim.body_state != "STAND" || !self.is_current_animation_idle() { return false; }
		if !self.is_happy() {
			self.play_available_refuse_animation();
			return false;
		}
		self.set_task(ActiveTask::Chopsticks);
		true
	}

	pub(super) fn update_active(&mut self, ctx: &Ctx) {
		match self.active {
			ActiveTask::Idle => self.update_idle(ctx),
			ActiveTask::Follow => self.update_follow(ctx),
			ActiveTask::Chopsticks => self.update_chopsticks(ctx),
		}
	}

	/// OpenViva's `IdleBehavior.OnUpdate`: idle variations and facing the player.
	fn update_idle(&mut self, ctx: &Ctx) {
		if !self.anim.has_balance { return; }

		// Turn towards the player from time to time.
		if self.look.target == LookTarget::PlayerHead && !self.puppet.is_move_active() {
			self.idle.face_target_timer -= ctx.dt;
			if self.idle.face_target_timer < 0.0 {
				self.idle.face_target_timer = 4.0 + Self::random() * 4.0;
				self.face_towards(Point3::from(ctx.player.head.translation.vector), 2.0);
			}
		}

		// Idle variations.
		if self.time > self.idle.next_variation_time {
			self.idle.next_variation_time = self.time + 10.0 + Self::random() * 15.0;
			if self.is_happy() && !self.tired && self.anim.body_state == "STAND" && self.is_current_animation_idle() {
				self.idle.idle_version = (self.idle.idle_version + 1) % 3;
				match self.idle.idle_version {
					1 => self.set_target_animation("STAND_HAPPY_IDLE2"),
					2 => self.set_target_animation("STAND_HAPPY_IDLE3"),
					_ => {},
				}
			}
		}

		// Sitting down in front of her invites her to play chopsticks.
		if self.is_player_sitting_in_front(ctx, true) && self.anim.body_state == "STAND" && self.is_current_animation_idle() {
			self.idle.chopsticks_invite_timer += ctx.dt;
			if self.idle.chopsticks_invite_timer > 2.0 {
				self.idle.chopsticks_invite_timer = 0.0;
				self.attempt_chopsticks();
			}
		} else {
			self.idle.chopsticks_invite_timer = 0.0;
		}
	}

	/// OpenViva's `FollowBehavior` (an `AutonomyMoveTo` towards the player): walks after the player and sits
	/// down when the player sits.
	fn update_follow(&mut self, ctx: &Ctx) {
		if !self.anim.has_balance || self.handhold.any_held() { return; }

		let player = ctx.player.floor_pos();
		let player_sitting = ctx.player.head.translation.vector.y - ctx.player.floor < 0.8;
		let distance = (player - self.floor_pos()).xz().norm();

		match self.anim.body_state.as_str() {
			"STAND" => {
				if distance > 1.0 {
					self.move_to(player, 0.8);
				} else if player_sitting && self.is_current_animation_idle() && !self.puppet.is_move_active() {
					self.face_towards(player, 1.0);
					self.set_target_animation("STAND_TO_SIT_FLOOR");
				} else if !self.puppet.is_move_active() {
					self.face_towards(player, 0.5);
				}
			},
			"FLOOR_SIT" => {
				if (!player_sitting || distance > 1.5) && self.is_current_animation_idle() {
					self.set_target_animation("FLOOR_SIT_TO_STAND");
				}
			},
			"SQUAT" => {
				if distance > 1.0 && self.is_current_animation_idle() {
					self.set_target_animation("SQUAT_TO_STAND");
				}
			},
			_ => {},
		}
	}

	pub(super) fn idle_on_animation_change(&mut self, _old: &str, _new: &str) {}

	/// OpenViva's `ChopsticksBehavior.IsPlayerSittingOnFloor` and `IsPlayerWithinChopsticksRange`.
	pub fn is_player_sitting_in_front(&self, ctx: &Ctx, test_bearings: bool) -> bool {
		let player = ctx.player.floor_pos();
		let me = self.floor_pos();
		let sq_dist = (player - me).xz().norm_squared();
		if !(0.15..2.4).contains(&sq_dist) { return false; }
		if test_bearings {
			if self.bearing(me, self.forward(), player).abs() > 40.0 { return false; }
			let player_forward = ctx.player.head.rotation * -crate::math::Vec3::z();
			let player_forward = vector!(player_forward.x, 0.0, player_forward.z);
			let to_me = (me - player).xz();
			if player_forward.xz().norm() > 0.1 && to_me.norm() > 0.01 {
				let angle = player_forward.xz().normalize().dot(&to_me.normalize()).clamp(-1.0, 1.0).acos().to_degrees();
				if angle > 40.0 { return false; }
			}
		}
		ctx.player.head.translation.vector.y - ctx.player.floor < 0.8
	}
}
