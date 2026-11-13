
//! Scripted interaction scenarios for testing the character without VR or manual input.
//!
//! Enabled with the `P39_MIKU_SCENARIO` environment variable (`headpat`, `rough_headpat`, `poke`, `tummy`,
//! `pickup`, `handhold`, `follow`, `wave`, `chopsticks`). The scenario moves the player's head and hands and
//! simulates grabs; progress is printed to stdout.

use std::time::Instant;
use rapier3d::dynamics::FixedJoint;

use crate::application::Hand;
use crate::component::ComponentBase;
use crate::component::hand::HandComponent;
use crate::component::model::mmd::BodyPart;
use crate::component::physics::joint::JointComponent;
use crate::math::{face_towards_lossy, Isometry3, Point3, Vec3};
use super::gestures::Gesture;
use super::{Character, Ctx};


pub struct Scenario {
	pub name: String,
	start: Instant,
	step: usize,
	grab: Option<crate::component::ComponentRef<JointComponent>>,
	/// Smoothed head position, followed by the patting hand.
	smooth_head: Option<Point3>,
}

impl Scenario {
	/// Returns `true` once, when step `index` is reached at time `at`.
	fn at(&mut self, index: usize, t: f32, at: f32) -> bool {
		if self.step == index && t >= at {
			self.step += 1;
			true
		} else {
			false
		}
	}
	
	pub fn from_env() -> Option<Self> {
		let name = std::env::var("P39_MIKU_SCENARIO").ok()?;
		println!("SCENARIO {name}");
		Some(Scenario { name, start: Instant::now(), step: 0, grab: None, smooth_head: None })
	}
}

impl Character {
	/// World point in front of the character: `forward` meters ahead, `left` to her left, `up` above the floor.
	fn scenario_point(&self, forward: f32, left: f32, up: f32) -> Point3 {
		self.floor_pos() + self.forward() * forward + self.left() * left + Vec3::y() * up
	}

	fn scenario_set_head(&self, ctx: &Ctx, position: Point3, look_at: Point3) {
		if let Some(head) = ctx.player.head_entity.get(ctx.app) {
			let rotation = face_towards_lossy(look_at - position);
			*head.state_mut().position = Isometry3::from_parts(position.coords.into(), rotation);
		}
	}

	fn scenario_set_hand(&self, ctx: &Ctx, hand: Hand, position: Point3) {
		if let Some(entity) = ctx.player.hand(hand).and_then(|h| h.entity.get(ctx.app)) {
			let rotation = entity.state().position.rotation;
			*entity.state_mut().position = Isometry3::from_parts(position.coords.into(), rotation);
		}
	}

	/// Grabs a body part with a player hand, like `HandComponent` does.
	fn scenario_grab(&self, ctx: &Ctx, scenario: &mut Scenario, hand: Hand, part: BodyPart) {
		let Some(hand_entity) = ctx.player.hand(hand).and_then(|h| h.entity.get(ctx.app)) else { return };
		let Some(body) = self.body(part).and_then(|b| b.entity.get(ctx.app)) else { return };
		let Some(component) = hand_entity.find_component_by_type::<HandComponent>() else { return };
		let offset = hand_entity.state().position.inverse() * *body.state().position;
		body.set_tag("Grabbed", component.as_cref());
		scenario.grab = Some(body.add_component(JointComponent::new(
			*FixedJoint::new().set_local_frame1(Isometry3::identity().into()).set_local_frame2(offset.into()),
			hand_entity.as_ref(),
		)));
	}

	fn scenario_release(&self, ctx: &Ctx, scenario: &mut Scenario) {
		if let Some(joint) = scenario.grab.take() {
			if let Some(body) = joint.entity().get(ctx.app) {
				body.unset_tag("Grabbed");
			}
			if let Some(joint) = joint.get(ctx.app) {
				joint.remove();
			}
		}
	}

	pub(super) fn run_scenario(&mut self, ctx: &Ctx, scenario: &mut Scenario) {
		let t = scenario.start.elapsed().as_secs_f32();
		let head = self.head_position();

		match scenario.name.as_str() {
			"headpat" | "rough_headpat" => {
				let rough = scenario.name == "rough_headpat";
				if t > 3.0 {
					self.scenario_set_head(ctx, self.scenario_point(0.45, 0.0, 1.55), head);
					// Like a player's hand, follow her head (the patting hand doesn't collide with her).
					let sphere = self.sphere_position(&self.config.headpat, ctx.app);
					let smooth = scenario.smooth_head.get_or_insert(sphere);
					*smooth += (sphere - *smooth) * (ctx.dt / 0.1).min(1.0);
					let top = if self.is_headpat_active() { *smooth + Vec3::y() * 0.02 } else { self.scenario_point(-0.03, 0.0, 1.5) };
					let (amplitude, speed) = if rough { (0.06, 45.0) } else { (0.04, 9.0) };
					let wiggle = if t > 4.0 && t < 14.0 { (t * speed).sin() * amplitude } else { 0.0 };
					let height = if t < 3.5 { 0.25 } else if t < 14.0 { 0.04 } else { 0.4 };
					self.scenario_set_hand(ctx, Hand::Right, top + self.left() * wiggle + Vec3::y() * height);
				}
			},
			"poke" => {
				let cycle = (t - 3.0).rem_euclid(2.0);
				if t > 3.0 {
					self.scenario_set_head(ctx, self.scenario_point(0.6, 0.0, 1.5), head);
					let face = self.sphere_position(&self.config.face, ctx.app);
					let distance = if cycle < 0.3 { 0.3 - cycle } else if cycle < 0.6 { 0.0 } else { 0.4 };
					self.scenario_set_hand(ctx, Hand::Right, face + self.forward() * distance + self.left() * 0.05);
				}
			},
			"tummy" => {
				let cycle = (t - 3.0).rem_euclid(0.8);
				if t > 3.0 {
					self.scenario_set_head(ctx, self.scenario_point(0.7, 0.0, 1.4), head);
					let tummy = self.sphere_position(&self.config.tummy, ctx.app);
					let distance = if cycle < 0.15 { 0.3 - cycle * 2.0 } else if cycle < 0.3 { 0.0 } else { 0.35 };
					self.scenario_set_hand(ctx, Hand::Right, tummy + self.forward() * distance);
				}
			},
			"pickup" => {
				if t < 3.0 { return; }
				self.scenario_set_head(ctx, self.scenario_point(1.2, 0.0, 1.5), head);
				let torso = self.body(BodyPart::Torso).map(|b| Point3::from(b.position.translation.vector)).unwrap_or(head);
				if scenario.at(0, t, 3.0) {
					self.scenario_set_hand(ctx, Hand::Right, torso);
				} else if scenario.at(1, t, 3.3) {
					self.scenario_grab(ctx, scenario, Hand::Right, BodyPart::Torso);
				}
				if scenario.step >= 2 && t < 7.0 {
					// Lift her up by the torso.
					let lift = ((t - 3.5) * 0.4).clamp(0.0, 0.5);
					if let Some(hand) = ctx.player.hand(Hand::Right) {
						let pos = Point3::from(hand.position.translation.vector);
						self.scenario_set_hand(ctx, Hand::Right, point!(pos.x, 1.1 + lift, pos.z));
					}
				}
				if scenario.at(2, t, 7.0) {
					self.scenario_release(ctx, scenario);
				}
			},
			"handhold" => {
				if t < 3.0 { return; }
				let hand = self.body(BodyPart::RightHand).map(|b| Point3::from(b.position.translation.vector)).unwrap_or(head);
				if scenario.at(0, t, 3.0) {
					self.scenario_set_head(ctx, self.scenario_point(0.5, -0.4, 1.5), head);
					self.scenario_set_hand(ctx, Hand::Left, hand);
				} else if scenario.at(1, t, 3.3) {
					self.scenario_grab(ctx, scenario, Hand::Left, BodyPart::RightHand);
				}
				if t > 6.0 && t < 14.0 {
					// Walk away, pulling her by the hand.
					let dir = vector!(1.0, 0.0, 0.0);
					let pos = Point3::from(ctx.player.head.translation.vector + dir * 0.5 * ctx.dt);
					self.scenario_set_head(ctx, pos, head);
					if let Some(h) = ctx.player.hand(Hand::Left) {
						let p = Point3::from(h.position.translation.vector + dir * 0.5 * ctx.dt);
						self.scenario_set_hand(ctx, Hand::Left, p);
					}
				}
				if scenario.at(2, t, 16.0) {
					self.scenario_release(ctx, scenario);
				}
			},
			"follow" => {
				if scenario.at(0, t, 3.0) {
					self.scenario_set_head(ctx, self.scenario_point(1.2, 0.0, 1.5), head);
				} else if scenario.at(1, t, 4.0) {
					self.on_gesture(Hand::Right, Gesture::Follow, ctx);
				}
				if t > 5.0 && t < 10.0 {
					let pos = Point3::from(ctx.player.head.translation.vector) + vector!(0.6, 0.0, 0.0) * ctx.dt;
					self.scenario_set_head(ctx, pos, head);
				}
				if t > 13.0 && t < 14.0 {
					// Sit down.
					let pos = ctx.player.head.translation.vector;
					self.scenario_set_head(ctx, point!(pos.x, 0.7, pos.z), self.floor_pos() + Vec3::y() * 0.7);
				}
			},
			"wave" => {
				if scenario.at(0, t, 3.0) {
					self.scenario_set_head(ctx, self.scenario_point(1.2, 0.0, 1.5), head);
				} else if scenario.at(1, t, 4.0) {
					self.on_gesture(Hand::Right, Gesture::Hello, ctx);
				}
			},
			"chopsticks" => {
				let sit = self.scenario_point(0.7, 0.0, 0.7);
				if t > 2.0 {
					self.scenario_set_head(ctx, sit, self.scenario_point(0.0, 0.0, 0.6));
					let rest_l = self.scenario_point(0.45, -0.15, 0.45);
					let rest_r = self.scenario_point(0.45, 0.15, 0.45);
					// Take the player's turns: tap her alive hand with an alive hand, every 5 seconds.
					let mut left = rest_l;
					let mut right = rest_r;
					if self.chopsticks.is_active() && (t % 5.0) > 3.5 && (t % 5.0) < 4.3 {
						let miku_right = self.chopsticks.miku.right <= 4;
						let target = self.body(if miku_right { BodyPart::RightHand } else { BodyPart::LeftHand })
						                 .map(|b| Point3::from(b.position.translation.vector));
						if let Some(target) = target {
							if self.chopsticks.player.right <= 4 { right = target; } else { left = target; }
						}
					}
					self.scenario_set_hand(ctx, Hand::Left, left);
					self.scenario_set_hand(ctx, Hand::Right, right);
				}
			},
			_ => {},
		}

	}
}
