use rand::seq::SliceRandom;

use super::super::anim::{LAYER_FACE, LAYER_LEGS, LAYER_TORSO};
use super::{Character, Ctx};
use super::data::{AnimEvent, AnimInfo, Transition, PRIORITY_NONE};


/// OpenViva's animation state machine (`Lily.AnimationSys`): animations have priorities and body state
/// requirements, and transition to the next animation when they end.
pub struct AnimState {
	pub current: String,
	pub target: String,
	pub last: String,
	pub current_priority: i32,
	pub highest_target_priority: i32,
	pub body_state: String,
	on_end_body_state: Option<String>,
	layer_states: [Option<String>; 3],
	/// Torso normalized time when events were last processed.
	event_time: f32,
	/// Balance is checked during the current animation.
	pub check_balance: bool,
	pub check_balance_transition: bool,
	pub has_balance: bool,
	/// Animation playback speed.
	pub speed: f32,
}

pub const NONE: &str = "NONE";

impl AnimState {
	pub fn new() -> Self {
		AnimState {
			current: NONE.into(),
			target: NONE.into(),
			last: NONE.into(),
			current_priority: PRIORITY_NONE,
			highest_target_priority: PRIORITY_NONE,
			body_state: "STAND".into(),
			on_end_body_state: None,
			layer_states: [None, None, None],
			event_time: 0.0,
			check_balance: true,
			check_balance_transition: true,
			has_balance: true,
			speed: 1.0,
		}
	}
}

impl Character {
	pub(super) fn init_animations(&mut self) {
		self.anim.target = self.last_returnable_idle();
		let target = self.anim.target.clone();
		self.update_transition(&target);
	}

	pub fn info(&self, name: &str) -> Option<&AnimInfo> {
		self.data.animations.get(name)
	}

	/// Requests an animation. Ignored if its body state doesn't match or a higher priority animation is requested.
	pub fn set_target_animation(&mut self, name: &str) {
		let Some(info) = self.data.animations.get(name) else {
			self.log(format!("Unknown animation {name}"));
			return;
		};
		if self.anim.body_state != info.condition || self.anim.highest_target_priority > info.priority {
			return;
		}
		if self.anim.current != name && info.priority >= self.anim.highest_target_priority {
			self.anim.highest_target_priority = info.priority;
			self.anim.target = name.to_string();
		}
	}

	pub fn override_clear_animation_priority(&mut self) {
		self.anim.highest_target_priority = PRIORITY_NONE;
		self.anim.current_priority = PRIORITY_NONE;
	}

	pub fn override_body_state(&mut self, state: &str) {
		if self.anim.body_state != state {
			self.log(format!("Body state {} -> {}", self.anim.body_state, state));
			self.anim.body_state = state.to_string();
		}
		self.anim.on_end_body_state = None;
	}

	/// Random animation of a set (eg. `IDLE_HAPPY`) of the current body state.
	pub fn animation_from_set(&self, set: &str) -> Option<String> {
		self.data.body_states.get(&self.anim.body_state)
		    .and_then(|state| state.sets.get(set))
		    .and_then(|anims| anims.choose(&mut rand::thread_rng()))
		    .cloned()
	}

	/// OpenViva's `GetAnimationFromSet(base, side, sets)`: picks one of `sets` variants (each with a right and
	/// left version) by offsetting the animation enum. Avoids replaying the current animation.
	pub fn animation_variant(&self, base: &str, side: usize, sets: usize) -> String {
		let sets = sets.max(1);
		let mut set = rand::thread_rng().gen_range_usize(sets);
		let mut anim = self.data.offset(base, set * 2 + side).map(str::to_string);
		if anim.as_deref() == Some(self.anim.current.as_str()) {
			set = (set + 1) % sets;
			anim = self.data.offset(base, set * 2 + side).map(str::to_string);
		}
		anim.unwrap_or_else(|| base.to_string())
	}

	/// OpenViva's `GetAnimationFromMood`: the next enum entry is the angry version.
	pub fn animation_from_mood(&self, base: &str) -> String {
		self.data.offset(base, if self.is_happy() { 0 } else { 1 }).unwrap_or(base).to_string()
	}

	pub fn last_returnable_idle(&self) -> String {
		let set = if self.tired { "IDLE_TIRED" } else if self.is_happy() { "IDLE_HAPPY" } else { "IDLE_ANGRY" };
		self.animation_from_set(set)
		    .or_else(|| self.animation_from_set("IDLE_HAPPY"))
		    .unwrap_or_else(|| "STAND_HAPPY_IDLE1".into())
	}

	pub fn is_current_animation_idle(&self) -> bool {
		self.info(&self.anim.current).map_or(false, AnimInfo::is_idle)
	}

	pub fn is_animation_changing_body_state(&self) -> bool {
		self.info(&self.anim.current).map_or(false, |info| info.condition != info.new_state)
	}

	/// Normalized time of the torso layer (OpenViva checks animation ends with it).
	pub fn torso_time(&self) -> f32 {
		self.animator.normalized_time(LAYER_TORSO)
	}

	fn allows_balance_check(&self, anim: &str) -> bool {
		let Some(info) = self.info(anim) else { return false };
		if info.disables_ragdoll_check() { return false; }
		let check = |state: &str| self.data.body_states.get(state).map_or(false, |s| s.check_balance);
		check(&info.condition) && check(&info.new_state)
	}

	/// OpenViva's `FixedUpdateAnimationGraph`: body state changes, animation transitions and events.
	pub(super) fn update_animation_graph(&mut self, ctx: &Ctx) {
		if let Some(info) = self.info(&self.anim.current)
		&& let Some(sync) = &info.torso_sync {
			let legs_time = self.animator.normalized_time(LAYER_LEGS);
			self.animator.set_param(&sync.clone(), legs_time);
		}

		if let Some(state) = self.anim.on_end_body_state.clone()
		&& self.torso_time() >= 1.0 {
			self.override_body_state(&state);
		}

		let transition = self.info(&self.anim.current).map(|i| i.transition.clone()).unwrap_or(Transition::Idle);
		let default = match transition {
			Transition::Idle => self.last_returnable_idle(),
			Transition::Next { next } => next,
			Transition::Loop => self.anim.current.clone(),
			Transition::Custom { handler } => match handler.as_str() {
				"postFacePokeAnimTransition" => self.poke.post_face_poke.clone().unwrap_or_else(|| self.last_returnable_idle()),
				"TransitionToPostTummyPokeAnim" => self.poke.post_tummy_poke.clone().unwrap_or_else(|| self.last_returnable_idle()),
				_ => self.last_returnable_idle(),
			},
		};
		self.update_transition(&default);

		self.process_events(ctx);
	}

	/// OpenViva's `UpdateAnimationTransition`.
	pub fn update_transition(&mut self, default: &str) {
		let time = self.torso_time();

		if self.anim.target == NONE {
			if time < 1.0 { return; }
			self.anim.target = default.to_string();
		} else if self.info(&self.anim.target).map_or(true, |info| info.priority < self.anim.current_priority) {
			return;
		}

		if self.anim.target == self.anim.current { return; }

		let target = self.anim.target.clone();
		let data = self.data.clone();
		let Some(info) = data.animations.get(&target) else {
			self.anim.target = NONE.into();
			return;
		};

		self.anim.current_priority = info.priority;
		self.anim.highest_target_priority = info.priority;

		for (layer, state) in [(LAYER_LEGS, &info.legs), (LAYER_TORSO, &info.torso), (LAYER_FACE, &info.face)] {
			if self.anim.layer_states[layer].as_ref() != Some(state) && self.animator.has_state(layer, state) {
				let mut offset = 0.0;
				if layer == LAYER_TORSO && info.torso_sync.is_some() {
					offset = self.animator.normalized_time(LAYER_LEGS);
				}
				self.animator.crossfade(layer, state, info.transition_time, offset);
				self.anim.layer_states[layer] = Some(state.clone());
			}
		}

		self.look.set_body_variables(info.logic.body_flags, info.logic.body_flag_percent, info.logic.body_flag_duration);
		self.look.set_eye_variables(info.logic.eye_flags, info.logic.eye_flag_duration);

		let old = self.anim.current.clone();
		self.on_animation_change(&old, &target);
	}

	/// OpenViva's `OnAnimationChange`.
	fn on_animation_change(&mut self, old: &str, new: &str) {
		self.anim.last = old.to_string();
		self.anim.current = new.to_string();
		self.anim.target = NONE.into();
		self.anim.event_time = -0.0001;

		if self.voice.bound_to_animation {
			self.stop_speaking();
			self.voice.bound_to_animation = false;
		}

		let data = self.data.clone();
		let info = &data.animations[new];
		if info.condition != info.new_state {
			self.override_body_state("NONE");
			self.anim.on_end_body_state = Some(info.new_state.clone());
		}

		self.anim.check_balance = self.allows_balance_check(new);
		self.anim.check_balance_transition = self.allows_balance_check(old) && self.anim.check_balance;
		self.anim.has_balance = info.new_state != "OFFBALANCE" && info.condition != "OFFBALANCE";

		self.log(format!("Animation {old} -> {new}"));

		self.headpat_on_animation_change(old, new);
		self.poke_on_animation_change(old, new);
		self.proximity_on_animation_change(old, new);
		self.chopsticks_on_animation_change(old, new);
		self.idle_on_animation_change(old, new);
		self.locomotion_on_animation_change(new);
	}

	/// Fires the current animation's events whose time was crossed since the last frame.
	fn process_events(&mut self, ctx: &Ctx) {
		let data = self.data.clone();
		let Some(info) = data.animations.get(&self.anim.current) else { return };
		if info.events.is_empty() { return; }

		let looping = info.torso.contains("_loop");
		let now = self.torso_time();
		let prev = self.anim.event_time;
		self.anim.event_time = now;

		let crossed = |time: f32| -> bool {
			if looping {
				let (a, b) = (prev, now);
				if b < a { return false; }
				let start = a.floor();
				let mut t = start + time;
				while t <= b {
					if t > a { return true; }
					t += 1.0;
				}
				false
			} else {
				time > prev && time <= now || (prev < 0.0 && time <= 0.0)
			}
		};

		for event in info.events.iter().filter(|e| crossed(e.time)) {
			self.handle_event(event, ctx);
		}
	}

	fn handle_event(&mut self, event: &AnimEvent, ctx: &Ctx) {
		match event.name.as_str() {
			"SPEAK" => {
				if let Some(line) = event.str(0) {
					self.speak(line, event.num(1) != 0.0, ctx);
				}
			},
			"SPEAK_INTERVALS" => {
				if let Some(line) = event.str(0) {
					self.speak_at_random_intervals(line, event.num(1).max(1.0), event.num(2), ctx);
				}
			},
			"SET_BODY_FLAGS" => self.look.set_body_variables(event.num(0) as u32, event.num(1), event.num(2)),
			"SET_EYE_FLAGS" => self.look.set_eye_variables(event.num(0) as u32, event.num(1)),
			"HEADPAT_PROPER_SOUND" => self.play_proper_headpat_sound(ctx),
			"REDISTRIBUTE_CHOPSTICKS_FINGERS" => self.redistribute_chopsticks_fingers(),
			_ => {},
		}
	}
}

trait GenRangeUsize {
	fn gen_range_usize(&mut self, max: usize) -> usize;
}

impl<R: rand::Rng> GenRangeUsize for R {
	fn gen_range_usize(&mut self, max: usize) -> usize {
		self.gen_range(0..max.max(1))
	}
}
