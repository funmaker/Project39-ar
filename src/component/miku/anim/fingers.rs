
//! Procedural finger posing.
//!
//! The MMD model has a full set of finger bones (thumb to little, three phalanges each), so finger
//! animations are applied to those bones instead of using blend shapes. This is the single system
//! used by the chopsticks game and by any other behavior that needs to control the fingers.
//!
//! The source rig's clips fully curl every finger around almost the same axis (the chopsticks idle
//! pose is a fist), which gives the canonical curl axes and angles below. A curl of 0 is an open
//! hand and 1 is the closed fist from the source animations.

use nalgebra::Unit;

use crate::math::{Rot3, Vec3};
use super::human;
use super::pose::{nlerp, Pose};


/// Local curl axis of a finger phalanx, in the (rest, identity rotation) humanoid bone space.
/// Indexed by `[side][finger][phalange]`, fingers in thumb..little order.
const CURL_AXES: [[[[f32; 3]; 3]; 5]; 2] = [
	// Left hand.
	[
		// Thumb.
		[[0.96, -0.25, -0.14], [0.98, -0.08, -0.16], [0.98, -0.04, -0.19]],
		// Index.
		[[0.15, -0.22, -0.96], [0.15, -0.22, -0.96], [0.15, -0.22, -0.96]],
		// Middle.
		[[-0.02, -0.21, -0.98], [-0.02, -0.21, -0.98], [-0.02, -0.21, -0.98]],
		// Ring.
		[[-0.20, -0.21, -0.96], [-0.20, -0.21, -0.96], [-0.20, -0.21, -0.96]],
		// Little.
		[[-0.40, -0.13, -0.91], [-0.40, -0.13, -0.91], [-0.40, -0.13, -0.91]],
	],
	// Right hand (mirrored across the body: X keeps its sign, Y and Z flip).
	[
		// Thumb.
		[[0.96, 0.25, 0.14], [0.98, 0.08, 0.16], [0.98, 0.04, 0.19]],
		// Index.
		[[0.15, 0.22, 0.96], [0.15, 0.22, 0.96], [0.15, 0.22, 0.96]],
		// Middle.
		[[-0.02, 0.21, 0.98], [-0.02, 0.21, 0.98], [-0.02, 0.21, 0.98]],
		// Ring.
		[[-0.20, 0.21, 0.96], [-0.20, 0.21, 0.96], [-0.20, 0.21, 0.96]],
		// Little.
		[[-0.40, 0.13, 0.91], [-0.40, 0.13, 0.91], [-0.40, 0.13, 0.91]],
	],
];

/// Curl angle (radians) of a fully closed finger phalanx.
const CURL_ANGLES: [[[f32; 3]; 5]; 2] = [
	[
		[22.0, 70.0, 70.0], // thumb
		[90.0, 90.0, 90.0], // index
		[90.0, 90.0, 90.0], // middle
		[90.0, 90.0, 90.0], // ring
		[90.0, 90.0, 90.0], // little
	],
	[
		[22.0, 70.0, 70.0],
		[90.0, 90.0, 90.0],
		[90.0, 90.0, 90.0],
		[90.0, 90.0, 90.0],
		[90.0, 90.0, 90.0],
	],
];

/// Per-finger curl of both hands, `[left, right][thumb..little]`, where 0 is extended and 1 is curled.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FingerCurl(pub [[f32; 5]; 2]);

impl FingerCurl {
	/// Fully open hands.
	pub fn open() -> Self {
		FingerCurl::default()
	}

	/// Builds a curl from per-finger extensions, `[left, right][thumb..little]`, where 1 is extended.
	pub fn from_extensions(extensions: [[f32; 5]; 2]) -> Self {
		let mut curl = [[0.0; 5]; 2];
		for side in 0..2 {
			for finger in 0..5 {
				curl[side][finger] = 1.0 - extensions[side][finger].clamp(0.0, 1.0);
			}
		}
		FingerCurl(curl)
	}

	pub fn lerp(&self, other: &FingerCurl, weight: f32) -> FingerCurl {
		let mut result = [[0.0; 5]; 2];
		for side in 0..2 {
			for finger in 0..5 {
				result[side][finger] = self.0[side][finger] + (other.0[side][finger] - self.0[side][finger]) * weight;
			}
		}
		FingerCurl(result)
	}

	/// Applies the curls to a pose, replacing the finger bone rotations.
	pub fn apply(&self, pose: &mut Pose) {
		for (side_id, side) in [human::Side::Left, human::Side::Right].into_iter().enumerate() {
			for finger in 0..5 {
				let curl = self.0[side_id][finger].clamp(0.0, 1.0);
				for phalange in 0..3 {
					let axis = Vec3::from(CURL_AXES[side_id][finger][phalange]);
					let axis = Unit::new_normalize(axis);
					let angle = CURL_ANGLES[side_id][finger][phalange].to_radians() * curl;
					pose.rotations[side.finger(finger, phalange)] = Rot3::from_axis_angle(&axis, angle);
				}
			}
		}
	}
}

/// Smoothly drives the fingers towards a target curl and blends the result into the animated pose.
///
/// Behaviors set a target with `set_target`; once no behavior needs the fingers anymore, `release`
/// fades the procedural pose out so the animation clips take over again.
pub struct FingerAnimator {
	curl: FingerCurl,
	target: FingerCurl,
	/// How strongly the procedural pose overrides the current animated pose.
	weight: f32,
	target_weight: f32,
	/// Curl interpolation speed (1/s).
	speed: f32,
	/// Blend weight speed (1/s).
	weight_speed: f32,
}

impl FingerAnimator {
	pub fn new() -> Self {
		FingerAnimator {
			curl: FingerCurl::open(),
			target: FingerCurl::open(),
			weight: 0.0,
			target_weight: 0.0,
			speed: 10.0,
			weight_speed: 12.0,
		}
	}

	/// Current (smoothed) curl.
	pub fn curl(&self) -> FingerCurl {
		self.curl
	}

	/// Whether the procedural fingers are visible (active or fading out).
	pub fn is_active(&self) -> bool {
		self.weight > 0.0001 || self.target_weight > 0.0001
	}

	/// Starts driving the fingers towards `target` at `speed` (curl units per second).
	pub fn set_target(&mut self, target: FingerCurl, speed: f32) {
		self.target = target;
		self.speed = speed.max(0.0);
		self.target_weight = 1.0;
	}

	/// Fades the procedural fingers out, returning control to the animation clips.
	pub fn release(&mut self) {
		self.target_weight = 0.0;
	}

	/// Immediately stops overriding the fingers.
	pub fn clear(&mut self) {
		self.weight = 0.0;
		self.target_weight = 0.0;
	}

	/// Advances the smoothed curl and blend weight.
	pub fn update(&mut self, dt: f32) {
		if dt <= 0.0 { return; }

		let curl_weight = (dt * self.speed).min(1.0);
		self.curl = self.curl.lerp(&self.target, curl_weight);

		let weight_step = (dt * self.weight_speed).min(1.0);
		self.weight += (self.target_weight - self.weight) * weight_step;
		if self.target_weight == 0.0 && self.weight < 0.001 {
			self.weight = 0.0;
		}
	}

	/// Blends the procedural fingers into `pose`.
	pub fn apply(&self, pose: &mut Pose) {
		if self.weight <= 0.0 { return; }

		let mut target = Pose::rest();
		self.curl.apply(&mut target);
		for side in [human::Side::Left, human::Side::Right] {
			for finger in 0..5 {
				for phalange in 0..3 {
					let bone = side.finger(finger, phalange);
					pose.rotations[bone] = nlerp(pose.rotations[bone], target.rotations[bone], self.weight);
				}
			}
		}
	}
}
