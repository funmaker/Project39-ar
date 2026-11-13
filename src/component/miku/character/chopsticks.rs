use crate::debug;
use crate::application::Hand;
use crate::math::{Color, Point3, Vec3};
use super::super::super::model::mmd::BodyPart;
use super::super::anim::FingerCurl;
use super::{Character, Ctx};
use super::idle::ActiveTask;
use super::look::LookTarget;


#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum GameState {
	None,
	MikuTurn,
	PlayerTurn,
}

/// Finger counts of both hands of a player. Counts above 4 mean the hand is out.
#[derive(Copy, Clone, Debug)]
pub struct Fingers {
	pub left: i32,
	pub right: i32,
}

impl Fingers {
	fn new() -> Self {
		Fingers { left: 1, right: 1 }
	}

	fn get(&self, right: bool) -> i32 {
		if right { self.right } else { self.left }
	}

	fn get_mut(&mut self, right: bool) -> &mut i32 {
		if right { &mut self.right } else { &mut self.left }
	}

	fn alive(&self, right: bool) -> bool {
		self.get(right) <= 4
	}

	/// Extended fingers of both hands, `[left, right][thumb..little]` (OpenViva's `updateTargetFingers`).
	fn extension(&self) -> [[f32; 5]; 2] {
		let mut result = [[0.0; 5]; 2];
		for (side, count) in [self.left, self.right].into_iter().enumerate() {
			if count <= 4 {
				for finger in 1..=count as usize {
					result[side][finger] = 1.0;
				}
			}
		}
		result
	}

	/// OpenViva's `AttemptRedistributeFingers`: splits the fingers evenly between the hands.
	fn redistribute(&mut self) -> bool {
		let left = if self.left > 4 { 0 } else { self.left };
		let right = if self.right > 4 { 0 } else { self.right };
		if left == right || (left - right).abs() == 1 { return false; }
		let half = (left + right) / 2;
		self.left = half;
		self.right = half;
		if half * 2 != left + right {
			self.left += 1;
		}
		true
	}
}

/// Miku reaching for one of the player's hands to tap it.
#[derive(Copy, Clone, Debug)]
struct Reach {
	miku_right: bool,
	player_hand: Hand,
	blend: f32,
	direction: f32,
}

/// The chopsticks finger game (OpenViva's `ChopsticksBehavior`): both players start with one finger
/// on each hand. On your turn, tap one of the opponent's hands with yours, adding your fingers to it.
/// A hand with 5 or more fingers is out; bringing your hands together splits your fingers evenly.
/// Whoever loses both hands loses the game.
pub struct Chopsticks {
	active: bool,
	state: GameState,
	pub player: Fingers,
	pub miku: Fingers,
	showed_hype: bool,
	wait_for_idle: bool,
	wait_turns_until_neutral: i32,
	react_confident: u8,
	react_worried: u8,
	valid_timer: f32,
	applied_turn_timer: f32,
	reach: Option<Reach>,
	player_hands_were_apart: bool,
}

impl Chopsticks {
	pub fn new() -> Self {
		Chopsticks {
			active: false,
			state: GameState::None,
			player: Fingers::new(),
			miku: Fingers::new(),
			showed_hype: false,
			wait_for_idle: false,
			wait_turns_until_neutral: 3,
			react_confident: 0,
			react_worried: 0,
			valid_timer: 0.0,
			applied_turn_timer: 0.0,
			reach: None,
			player_hands_were_apart: true,
		}
	}

	/// The game is being played (not just sitting down for it).
	pub fn is_active(&self) -> bool {
		self.active
	}
}

impl Character {
	fn begin_chopsticks_game(&mut self) {
		self.log("Chopsticks: new game");
		self.chopsticks.active = true;
		self.chopsticks.player = Fingers::new();
		self.chopsticks.miku = Fingers::new();
		self.chopsticks.wait_for_idle = true;
		self.chopsticks.state = GameState::MikuTurn;
		self.chopsticks.react_confident = 0;
		self.chopsticks.react_worried = 0;
		self.chopsticks.applied_turn_timer = 0.0;
		self.chopsticks.reach = None;
	}

	fn finish_chopsticks_game(&mut self) {
		self.chopsticks.state = GameState::None;
		self.chopsticks.showed_hype = false;
		self.chopsticks.reach = None;
	}

	pub(super) fn chopsticks_deactivate(&mut self) {
		self.chopsticks.active = false;
		self.chopsticks.state = GameState::None;
		self.chopsticks.reach = None;
		self.fingers.release();
		self.puppet.hand_reach = [None, None];
		self.animator.set_param("ChopsticksReach", 0.0);
	}

	pub(super) fn redistribute_chopsticks_fingers(&mut self) {
		if self.chopsticks.miku.redistribute() {
			self.chopsticks.applied_turn_timer = 0.0001;
		}
	}

	/// Player's hand position, where the fingers are.
	fn player_hand_position(&self, ctx: &Ctx, hand: Hand) -> Option<Point3> {
		ctx.player.hand(hand).map(|h| Point3::from(h.position.translation.vector))
	}

	/// Center of Miku's hand: a bit past the wrist, along the forearm.
	fn miku_hand_position(&self, right: bool) -> Option<Point3> {
		let (hand, forearm) = if right { (BodyPart::RightHand, BodyPart::RightForearm) } else { (BodyPart::LeftHand, BodyPart::LeftForearm) };
		let wrist = Point3::from(self.body(hand)?.position.translation.vector);
		let elbow = self.body(forearm).map_or(wrist, |b| Point3::from(b.position.translation.vector));
		Some(wrist + (wrist - elbow).try_normalize(0.0001).unwrap_or(Vec3::zeros()) * 0.06)
	}
	
	pub(super) fn update_chopsticks(&mut self, ctx: &Ctx) {
		if !self.anim.has_balance {
			self.set_task(ActiveTask::Idle);
			return;
		}

		if !self.chopsticks.active {
			match self.anim.body_state.as_str() {
				"STAND" => {
					self.set_target_animation("STAND_TO_SIT_FLOOR");
					self.face_towards(ctx.player.floor_pos(), 2.0);
				},
				"FLOOR_SIT" => {
					if self.anim.current == "FLOOR_SIT_LOCOMOTION_HAPPY" {
						if !self.chopsticks.showed_hype {
							self.set_target_animation("FLOOR_SIT_HYPE");
						} else {
							self.set_target_animation("FLOOR_SIT_CHOPSTICKS_IN");
						}
					}
				},
				"NONE" => {},
				_ => self.set_task(ActiveTask::Idle),
			}
		} else if self.player_left_chopsticks_location(ctx) {
			self.log("Chopsticks: player left");
			self.set_target_animation("FLOOR_SIT_TO_STAND");
		} else {
			self.update_chopsticks_game(ctx);
		}

		// Animate Miku's fingers towards their counts. The finger animator drives the actual
		// finger bones, so both hands (and any other finger animation) use the same system.
		if self.chopsticks.active {
			self.fingers.set_target(FingerCurl::from_extensions(self.chopsticks.miku.extension()), 8.0);
		}

		self.draw_chopsticks_hud(ctx);
	}

	fn player_left_chopsticks_location(&mut self, ctx: &Ctx) -> bool {
		if !self.is_player_sitting_in_front(ctx, false) {
			self.chopsticks.valid_timer += ctx.dt;
			if self.chopsticks.valid_timer > 2.0 {
				self.chopsticks.valid_timer = 0.0;
				return true;
			}
		} else {
			self.chopsticks.valid_timer = 0.0;
		}
		false
	}

	fn update_chopsticks_game(&mut self, ctx: &Ctx) {
		match self.chopsticks.state {
			GameState::None => {},
			GameState::MikuTurn => {
				if self.chopsticks.reach.is_none() {
					if self.chopsticks.applied_turn_timer == 0.0 {
						if !self.chopsticks.wait_for_idle {
							if self.chopsticks.wait_turns_until_neutral == 0 {
								self.chopsticks.wait_turns_until_neutral = 4 + (Self::random() * 6.0) as i32;
								self.set_target_animation("FLOOR_SIT_CHOPSTICKS_NEUTRAL");
								self.chopsticks.wait_for_idle = true;
							} else {
								self.chopsticks.wait_turns_until_neutral -= 1;
								self.miku_take_turn(ctx);
							}
						}
					} else {
						self.react_confident();
						self.end_chopsticks_turn();
					}
				}
				self.update_reach(ctx);
			},
			GameState::PlayerTurn => {
				if self.chopsticks.applied_turn_timer == 0.0 {
					self.wait_for_player_tap(ctx);
				} else {
					self.chopsticks.applied_turn_timer += ctx.dt;
					if self.chopsticks.applied_turn_timer > 0.5 && !self.player_hand_near_miku_hands(ctx, 0.15) && !self.chopsticks.wait_for_idle {
						self.react_worried();
						self.end_chopsticks_turn();
					}
				}
			},
		}
	}

	fn end_chopsticks_turn(&mut self) {
		match self.chopsticks.state {
			GameState::MikuTurn => {
				if !self.chopsticks.player.alive(false) && !self.chopsticks.player.alive(true) {
					self.log("Chopsticks: Miku won");
					self.set_target_animation("FLOOR_SIT_CHOPSTICKS_WIN");
					self.finish_chopsticks_game();
				} else {
					self.chopsticks.state = GameState::PlayerTurn;
				}
			},
			GameState::PlayerTurn => {
				if !self.chopsticks.miku.alive(false) && !self.chopsticks.miku.alive(true) {
					self.log("Chopsticks: player won");
					self.set_target_animation("FLOOR_SIT_CHOPSTICKS_LOSE");
					self.finish_chopsticks_game();
				} else {
					self.chopsticks.state = GameState::MikuTurn;
				}
			},
			GameState::None => {},
		}
		self.chopsticks.applied_turn_timer = 0.0;
	}

	/// OpenViva's `WaitUntilTapHandShinobuTurn`: picks a move, preferring ones that knock out a player's hand.
	fn miku_take_turn(&mut self, ctx: &Ctx) {
		if (self.floor_pos() - ctx.player.floor_pos()).xz().norm() > 1.8 { return; }

		let miku = self.chopsticks.miku;
		let player = self.chopsticks.player;

		if !miku.alive(false) || !miku.alive(true) {
			let can_split = (miku.right > 1 && miku.right <= 4) || (miku.left > 1 && miku.left <= 4);
			if can_split {
				self.set_target_animation("FLOOR_SIT_CHOPSTICKS_REDISTRIBUTE");
				self.chopsticks.wait_for_idle = true;
				return;
			}
		}

		// Moves: (miku right hand, player right hand)
		let moves = [(false, false), (false, true), (true, false), (true, true)];
		let valid = |&(miku_right, player_right): &(bool, bool)| miku.alive(miku_right) && player.alive(player_right);
		let kills: Vec<_> = moves.iter().filter(|m| valid(m) && miku.get(m.0) + player.get(m.1) > 4).copied().collect();
		let candidates: Vec<_> = if kills.is_empty() { moves.iter().filter(|m| valid(m)).copied().collect() } else { kills };
		if candidates.is_empty() { return; }

		let (miku_right, player_right) = candidates[(Self::random() * candidates.len() as f32) as usize % candidates.len()];
		let player_hand = if player_right { Hand::Right } else { Hand::Left };
		self.chopsticks.reach = Some(Reach { miku_right, player_hand, blend: 0.0, direction: 1.0 });
		self.set_look_at(LookTarget::PlayerHand(player_hand));
		self.look.set_awareness_timeout(1.5);
	}

	/// OpenViva's `UpdateReachToPlayerHandAnim`: reaches out, taps the player's hand and pulls back.
	fn update_reach(&mut self, ctx: &Ctx) {
		self.puppet.hand_reach = [None, None];
		let Some(mut reach) = self.chopsticks.reach else {
			self.animator.set_param("ChopsticksReach", 0.0);
			return;
		};

		let Some(target) = self.player_hand_position(ctx, reach.player_hand) else {
			self.chopsticks.reach = None;
			return;
		};
		let hand = self.miku_hand_position(reach.miku_right).unwrap_or(target);
		let tap_speed = 2.0 - (hand - target).norm().max(0.0);
		reach.blend += ctx.dt * tap_speed * reach.direction;

		if reach.blend >= 1.0 {
			reach.blend = 1.0;
			reach.direction = -0.75;
			// Tap: add her fingers to the player's hand.
			let count = self.chopsticks.miku.get(reach.miku_right);
			*self.chopsticks.player.get_mut(reach.player_hand == Hand::Right) += count;
			self.chopsticks.applied_turn_timer = ctx.dt;
			self.log(format!("Chopsticks: Miku tapped, player {:?}", self.chopsticks.player));
		} else if reach.blend <= 0.0 {
			self.chopsticks.reach = None;
			self.animator.set_param("ChopsticksReach", 0.0);
			return;
		}

		let eased = if reach.blend < 0.5 { 2.0 * reach.blend * reach.blend } else { 1.0 - (-2.0 * reach.blend + 2.0).powi(2) / 2.0 };
		self.animator.set_param("ChopsticksReach", eased);
		let slot = reach.miku_right as usize;
		self.puppet.hand_reach[slot] = Some((target + Vec3::y() * 0.03, eased * 0.85));
		let parts = if reach.miku_right { [BodyPart::RightForearm, BodyPart::RightArm] } else { [BodyPart::LeftForearm, BodyPart::LeftArm] };
		for part in parts {
			self.puppet.part_pin.insert(part, 1.0 - eased * 0.8);
		}
		self.chopsticks.reach = Some(reach);
	}

	fn player_hand_near_miku_hands(&self, ctx: &Ctx, distance: f32) -> bool {
		[Hand::Left, Hand::Right].into_iter().any(|player_hand| {
			let Some(pos) = self.player_hand_position(ctx, player_hand) else { return false };
			[false, true].into_iter().any(|miku_right| {
				self.miku_hand_position(miku_right).map_or(false, |hand| (hand - pos).norm() < distance)
			})
		})
	}

	/// OpenViva's `WaitUntilTapHandPlayerTurn`: the player taps one of her hands, or brings both hands together
	/// to split the fingers.
	fn wait_for_player_tap(&mut self, ctx: &Ctx) {
		let left = self.player_hand_position(ctx, Hand::Left);
		let right = self.player_hand_position(ctx, Hand::Right);

		for player_hand in [Hand::Right, Hand::Left] {
			let player_right = player_hand == Hand::Right;
			if !self.chopsticks.player.alive(player_right) { continue; }
			let Some(pos) = self.player_hand_position(ctx, player_hand) else { continue };
			for miku_right in [true, false] {
				if !self.chopsticks.miku.alive(miku_right) { continue; }
				let Some(hand) = self.miku_hand_position(miku_right) else { continue };
				if (hand - pos).norm() < 0.1 {
					let count = self.chopsticks.player.get(player_right);
					*self.chopsticks.miku.get_mut(miku_right) += count;
					self.set_target_animation(if miku_right { "FLOOR_SIT_CHOPSTICKS_RECEIVE_RIGHT" } else { "FLOOR_SIT_CHOPSTICKS_RECEIVE_LEFT" });
					self.set_look_at(LookTarget::Point(hand));
					self.look.set_awareness_timeout(1.0);
					self.chopsticks.applied_turn_timer = ctx.dt;
					self.log(format!("Chopsticks: player tapped, Miku {:?}", self.chopsticks.miku));
					return;
				}
			}
		}

		if let (Some(left), Some(right)) = (left, right) {
			let together = (left - right).norm_squared() <= 0.01;
			if together && self.chopsticks.player_hands_were_apart && self.chopsticks.player.redistribute() {
				self.log(format!("Chopsticks: player split, {:?}", self.chopsticks.player));
				self.chopsticks.applied_turn_timer = ctx.dt;
				self.chopsticks.react_confident = 0;
			}
			self.chopsticks.player_hands_were_apart = !together;
		}
	}

	fn react_worried(&mut self) {
		if Self::random() < 0.4 { return; }
		if self.chopsticks.miku.right >= 4 && self.chopsticks.react_worried & 2 == 0 {
			self.chopsticks.react_worried |= 2;
			self.set_target_animation("FLOOR_SIT_CHOPSTICKS_WORRIED");
			self.chopsticks.wait_for_idle = true;
		} else if self.chopsticks.miku.left >= 4 && self.chopsticks.react_worried & 1 == 0 {
			self.chopsticks.react_worried |= 1;
			self.set_target_animation("FLOOR_SIT_CHOPSTICKS_WORRIED");
			self.chopsticks.wait_for_idle = true;
		}
	}

	fn react_confident(&mut self) {
		if Self::random() < 0.4 { return; }
		if self.chopsticks.player.right > 4 && self.chopsticks.react_confident & 2 == 0 {
			self.chopsticks.react_confident |= 2;
			self.set_target_animation("FLOOR_SIT_CHOPSTICKS_CONFIDENT");
			self.chopsticks.wait_for_idle = true;
		} else if self.chopsticks.player.left > 4 && self.chopsticks.react_confident & 1 == 0 {
			self.chopsticks.react_confident |= 1;
			self.set_target_animation("FLOOR_SIT_CHOPSTICKS_CONFIDENT");
			self.chopsticks.wait_for_idle = true;
		}
	}

	/// Shows the player's finger counts next to their hands. The player's hands are static meshes
	/// (modeled fingers but no bone rig), so the count is drawn as text; Miku's own fingers use the
	/// real finger bones via `FingerAnimator`.
	fn draw_chopsticks_hud(&self, ctx: &Ctx) {
		if !self.chopsticks.active { return; }
		for (hand, right) in [(Hand::Left, false), (Hand::Right, true)] {
			let Some(pos) = self.player_hand_position(ctx, hand) else { continue };
			let count = self.chopsticks.player.get(right);
			let text = if count > 4 { "X".to_string() } else { count.to_string() };
			let color = if self.chopsticks.state == GameState::PlayerTurn { Color::GREEN } else { Color::WHITE };
			debug::draw_text(text, pos + Vec3::y() * 0.08, debug::DebugOffset::center(0.0, 0.0), 48.0, color);
		}
	}

	pub(super) fn chopsticks_on_animation_change(&mut self, old: &str, new: &str) {
		match old {
			"FLOOR_SIT_HYPE" => self.chopsticks.showed_hype = true,
			"FLOOR_SIT_CHOPSTICKS_WIN" | "FLOOR_SIT_CHOPSTICKS_LOSE" if self.active == ActiveTask::Chopsticks => {
				self.set_target_animation("FLOOR_SIT_CHOPSTICKS_IN");
			},
			_ => {},
		}
		if self.active != ActiveTask::Chopsticks { return; }
		match new {
			"FLOOR_SIT_CHOPSTICKS_IDLE" => self.chopsticks.wait_for_idle = false,
			"FLOOR_SIT_CHOPSTICKS_IN" => self.begin_chopsticks_game(),
			"FLOOR_SIT_TO_STAND" => {
				self.chopsticks_deactivate();
				self.set_task(ActiveTask::Idle);
			},
			_ => {},
		}
	}
}
