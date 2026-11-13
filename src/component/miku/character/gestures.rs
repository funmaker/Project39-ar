use nalgebra::Unit;

use crate::application::{Hand, Key};
use crate::debug;
use crate::math::{Color, Isometry3, Point3, Rot3, Vec3};
use super::{Character, Ctx};


/// Player gestures recognized by the character (OpenViva's `ObjectFingerPointer.Gesture`).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Gesture {
	/// Waving.
	Hello,
	/// Beckoning ("come here").
	Follow,
	/// Holding a hand out, palm up.
	PresentStart,
	PresentEnd,
}

impl Gesture {
	/// Short text shown as a visual confirmation.
	fn hint(self) -> Option<&'static str> {
		match self {
			Gesture::Hello => Some("Hello!"),
			Gesture::Follow => Some("Come here!"),
			Gesture::PresentStart => Some("Offer"),
			Gesture::PresentEnd => None,
		}
	}

	/// Sound effect name in `voice.json`.
	fn sfx(self) -> Option<&'static str> {
		match self {
			Gesture::Hello => Some("gesture_hello"),
			Gesture::Follow => Some("gesture_follow"),
			Gesture::PresentStart => Some("gesture_present"),
			Gesture::PresentEnd => None,
		}
	}
}

/// A recognized gesture shown to the player for a moment.
#[derive(Copy, Clone, Debug)]
struct GestureHint {
	gesture: Gesture,
	position: Point3,
	time: f32,
}

/// Per-hand gesture recognizer state (OpenViva's `ObjectFingerPointer.GestureHand`).
#[derive(Default, Clone)]
struct GestureHand {
	follow_step: i32,
	follow_last_swing: f32,
	follow_target_sign: i32,
	follow_start: f32,
	follow_last: f32,
	present_last_palm_up: f32,
	present_wait_for_reset: f32,
	hello_step: i32,
	hello_last_wave: f32,
	hello_target_sign: i32,
	hello_start: f32,
	hello_last: f32,
}

/// Recognizes gestures from the motion of the player's hands relative to their head.
///
/// The hand axes follow OpenViva's hand bones mapped to a VR controller: `up` points along the fingers
/// (controller -Z), `right` is the controller's +X and `forward` the back of the hand (controller +Y).
/// Number of gesture target orientations tested per hand: the raw controller axes and the
/// controller axes rotated around the finger axis by the prefab's gesture target yaw (±120°).
const GESTURE_CANDIDATES: usize = 3;
/// `gestureTarget` local yaw relative to the hand bone in OpenViva's player prefab.
const GESTURE_TARGET_YAW: f32 = 120.0 * std::f32::consts::PI / 180.0;

pub struct GestureDetector {
	hands: [[GestureHand; GESTURE_CANDIDATES]; 2],
	hint: Option<GestureHint>,
}

impl GestureDetector {
	pub fn new() -> Self {
		GestureDetector {
			hands: [std::array::from_fn(|_| GestureHand::default()),
			        std::array::from_fn(|_| GestureHand::default())],
			hint: None,
		}
	}
}

#[derive(Clone, Copy)]
struct HandAxes {
	up: Vec3,
	right: Vec3,
	forward: Vec3,
	position: Vec3,
}

impl HandAxes {
	/// Rotates the in-hand axes around the finger axis, as OpenViva's `gestureTarget` does.
	fn with_yaw(self, angle: f32) -> Self {
		let rotation = Rot3::from_axis_angle(&Unit::new_normalize(self.up), angle);
		HandAxes {
			right: rotation * self.right,
			forward: rotation * self.forward,
			..self
		}
	}
}

fn hand_axes(head: &Isometry3, hand: &Isometry3, left: bool) -> HandAxes {
	let to_head = head.rotation.inverse();
	let sign = if left { -1.0 } else { 1.0 };
	HandAxes {
		up: to_head * (hand.rotation * -Vec3::z()),
		right: to_head * (hand.rotation * Vec3::x()) * sign,
		forward: to_head * (hand.rotation * Vec3::y()),
		position: (head.inverse() * hand).translation.vector,
	}
}

impl GestureHand {
	/// OpenViva's `AttemptHello`: hand pointing up, waving side to side.
	fn attempt_hello(&mut self, axes: &HandAxes, time: f32) -> bool {
		if axes.up.y > 0.7 && axes.right.x.abs() > 0.5 {
			let mut sign = (axes.up.x - self.hello_last).signum() as i32;
			if sign == 0 { sign = -1; }
			if self.hello_step == 0 {
				self.hello_target_sign = sign;
				self.hello_step += 1;
				self.hello_last_wave = time;
				self.hello_start = axes.up.x;
			} else if sign != self.hello_target_sign {
				self.hello_target_sign = sign;
				let distance = (axes.up.x - self.hello_start).abs();
				if (1.2 - distance).abs() < 0.8 && (0.5 - (time - self.hello_last_wave)).abs() < 0.44 {
					self.hello_step += 1;
					if self.hello_step > 4 {
						self.hello_step = 0;
						return true;
					}
				} else {
					self.hello_step = 0;
				}
				self.hello_last_wave = time;
				self.hello_start = axes.up.x;
			}
			self.hello_last = axes.up.x;
		} else {
			self.hello_step = 0;
		}
		false
	}

	/// OpenViva's `AttemptFollow`: palm up, swinging the hand towards the face.
	fn attempt_follow(&mut self, axes: &HandAxes, time: f32) -> bool {
		if self.follow_step < 4 {
			if (-0.4 - axes.up.x).abs() < 0.8 {
				if -axes.forward.z > 0.0 {
					self.follow_last_swing = time;
					self.follow_step = 0;
					return false;
				}
				let mut sign = (axes.forward.y - self.follow_last).signum() as i32;
				if sign == 0 { sign = -1; }
				if self.follow_step == 0 {
					self.follow_target_sign = sign;
					self.follow_step += 1;
					self.follow_last_swing = time - 0.1;
					self.follow_start = axes.forward.y;
				} else {
					let distance = (self.follow_start - axes.forward.y).abs();
					if sign != self.follow_target_sign && distance > 0.15 {
						self.follow_target_sign = sign;
						if time - self.follow_last_swing > 0.8 {
							self.follow_step = 0;
						} else if time - self.follow_last_swing > 0.1 && distance > 0.2 {
							self.follow_step += 1;
							self.follow_last_swing = time;
							self.follow_start = axes.forward.y;
						}
					}
				}
				self.follow_last = axes.forward.y;
			} else {
				self.follow_last_swing = time;
				self.follow_step = 0;
			}
		} else {
			self.follow_step = 0;
			self.present_wait_for_reset = 1.0;
			return true;
		}
		false
	}

	/// OpenViva's `AttemptPresent`: palm up in front of the face.
	fn attempt_present(&mut self, axes: &HandAxes, time: f32, dt: f32) -> Option<bool> {
		if -axes.forward.y > 0.7 {
			let local = axes.position;
			if local.x.abs() < 0.3 && local.y.abs() < 0.4 && -local.z > 0.1 && -axes.up.z > 0.6 {
				if time - self.present_last_palm_up > 0.5 && self.present_wait_for_reset <= 0.0 {
					self.present_wait_for_reset = 0.3;
					return Some(true);
				}
			} else {
				self.present_last_palm_up = time;
			}
		} else {
			self.present_last_palm_up = time;
			if self.present_wait_for_reset > 0.0 {
				self.present_wait_for_reset -= dt;
				if self.present_wait_for_reset <= 0.0 {
					return Some(false);
				}
			} else {
				self.present_wait_for_reset -= dt;
			}
		}
		None
	}
}

impl Character {
	pub(super) fn update_gestures(&mut self, ctx: &Ctx) {
		let mut gestures = vec![];

		// Gestures can't be done precisely with novr hands, so they also have keyboard shortcuts.
		let keyboard = &ctx.app.input.keyboard;
		if keyboard.down(Key::KeyH) {
			let position = ctx.player.hand(Hand::Right).map_or_else(|| Point3::from(ctx.player.head.translation.vector), |h| Point3::from(h.position.translation.vector));
			gestures.push((Hand::Right, Gesture::Hello, position));
		}
		if keyboard.down(Key::KeyF) {
			let position = ctx.player.hand(Hand::Right).map_or_else(|| Point3::from(ctx.player.head.translation.vector), |h| Point3::from(h.position.translation.vector));
			gestures.push((Hand::Right, Gesture::Follow, position));
		}

		if ctx.app.vr.is_some() {
			for hand in &ctx.player.hands {
				let slot = match hand.hand { Hand::Left => 0, Hand::Right => 1 };
				let base = hand_axes(&ctx.player.head, &hand.position, hand.hand == Hand::Left);
				// Try the raw controller axes and OpenViva's gesture target yaw in both
				// directions. OpenVR's handedness conversion can mirror the yaw, and getting
				// this wrong is why gestures didn't trigger before.
				let candidates = [base, base.with_yaw(GESTURE_TARGET_YAW), base.with_yaw(-GESTURE_TARGET_YAW)];
				let position = Point3::from(hand.position.translation.vector);

				let mut fired = None;
				for (candidate, axes) in candidates.iter().enumerate() {
					let detector = &mut self.gestures.hands[slot][candidate];
					if detector.attempt_hello(axes, self.time) {
						fired = Some(Gesture::Hello);
						break;
					}
					if detector.attempt_follow(axes, self.time) {
						fired = Some(Gesture::Follow);
						break;
					}
					if candidate == 0 {
						match detector.attempt_present(axes, self.time, ctx.dt) {
							Some(true) => gestures.push((hand.hand, Gesture::PresentStart, position)),
							Some(false) => gestures.push((hand.hand, Gesture::PresentEnd, position)),
							None => {},
						}
					}
				}
				if let Some(gesture) = fired {
					gestures.push((hand.hand, gesture, position));
				}
			}
		}

		for (hand, gesture, position) in gestures {
			self.fire_gesture(hand, gesture, position, ctx);
		}

		self.draw_gesture_hint();
	}

	/// Plays the confirmation sound/hint and dispatches a recognized gesture.
	pub(super) fn fire_gesture(&mut self, hand: Hand, gesture: Gesture, position: Point3, ctx: &Ctx) {
		self.log(format!("Gesture {gesture:?}"));
		if let Some(sfx) = gesture.sfx() {
			self.play_sfx(sfx, 1.0, ctx);
		}
		if gesture.hint().is_some() {
			self.gestures.hint = Some(GestureHint { gesture, position, time: self.time });
		}
		self.on_gesture(hand, gesture, ctx);
	}

	/// Draws the recognized gesture near the player's hand for a moment.
	fn draw_gesture_hint(&mut self) {
		let Some(hint) = self.gestures.hint else { return };
		let age = self.time - hint.time;
		const DURATION: f32 = 1.2;
		if age > DURATION {
			self.gestures.hint = None;
			return;
		}

		let alpha = 1.0 - age / DURATION;
		let text = hint.gesture.hint().unwrap_or_default();
		let base = hint.position + Vec3::y() * (0.12 + age * 0.12);
		debug::draw_text(text, base, debug::DebugOffset::center(0.0, 0.0), 32.0, Color::CYAN.opactiy(alpha));
		debug::draw_point(hint.position, 10.0 + age * 40.0, Color::CYAN.opactiy(alpha * 0.8));
		for i in 0..5 {
			let drift = Vec3::y() * (age * 0.3 + i as f32 * 0.035);
			debug::draw_point(hint.position + drift, 4.0, Color::CYAN.opactiy(alpha * (1.0 - i as f32 / 5.0) * 0.7));
		}
	}
}
