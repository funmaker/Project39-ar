
//! Port of OpenViva's character ("Lily"/Shinobu) logic: physics based animation of the ragdoll,
//! the animation state machine, voice and the player interactions (headpats, pokes, hand holding,
//! kissing, hugging, picking up, gestures, following and the chopsticks game).
//!
//! The character is split across files like OpenViva's partial `Lily` class: every file adds an
//! `impl Character` block with one area of logic.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use anyhow::Result;
use rand::Rng;
use serde_derive::Deserialize;

mod animation;
mod chopsticks;
mod data;
mod gestures;
mod handhold;
mod headpat;
mod hug;
mod idle;
mod kiss;
mod look;
mod poke;
mod proximity;
mod puppet;
mod scenario;
mod voice;

use crate::application::{Application, Entity, EntityRef, Hand};
use crate::component::{ComponentBase, ComponentRef};
use crate::component::hand::HandComponent;
use crate::component::model::MMDModel;
use crate::component::model::mmd::BodyPart;
use crate::math::{Isometry3, Point3, Rot3, Vec3};
use crate::renderer::assets_manager::JsonAsset;
use super::anim::{human, Animator, ControllerDesc, FingerAnimator, Pose, Retarget, SkeletonDesc};
pub use animation::AnimState;
pub use chopsticks::Chopsticks;
pub use data::{AnimationData, VoiceData};
pub use gestures::GestureDetector;
pub use handhold::Handhold;
pub use headpat::Headpat;
pub use hug::Hug;
pub use idle::{ActiveTask, Idle};
pub use kiss::Kiss;
pub use look::LookAt;
pub use poke::Poke;
pub use proximity::Proximity;
pub use puppet::Puppet;
pub use voice::Voice;


/// `[character]` section of a model's `model.toml`, describing how to animate the model as an OpenViva character.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct ModelCharacterConfig {
	pub character: Option<CharacterConfig>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CharacterConfig {
	/// Directory with the imported OpenViva data, relative to `assets/`.
	#[serde(default = "default_data_dir")]
	pub data: String,
	/// Humanoid bone names (see `anim::human`) mapped to MMD bone ids.
	pub bones: HashMap<String, usize>,
	/// Source blend shapes mapped to lists of `[MMD morph id, weight]`.
	#[serde(default)]
	pub morphs: HashMap<String, Vec<(usize, f32)>>,
	/// Top of the head, where headpats happen. Model space at rest.
	pub headpat: SphereConfig,
	/// Face, where pokes happen.
	pub face: SphereConfig,
	/// Tummy, where pokes make her laugh.
	pub tummy: SphereConfig,
}

fn default_data_dir() -> String { "openviva".into() }

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct SphereConfig {
	/// MMD bone the sphere follows.
	pub bone: usize,
	/// Center in model space, at rest.
	pub center: [f32; 3],
	pub radius: f32,
}

/// A ragdoll rigid body driven by the animation.
#[derive(Clone, Debug)]
pub struct Body {
	pub entity: EntityRef,
	pub part: BodyPart,
	/// MMD bone whose animated transform moves the body.
	pub bone: usize,
	/// Model space rest pose.
	pub rest: Isometry3,
	/// Parent body in the ragdoll.
	pub parent: Option<usize>,
	pub position: Isometry3,
	pub velocity: Vec3,
	pub angular_velocity: Vec3,
	/// Player hand holding the body.
	pub grabbed: Option<Hand>,
	/// Animated world pose the body is driven towards.
	pub target: Isometry3,
}

/// Player's hand, as seen by the character.
#[derive(Clone, Debug)]
pub struct PlayerHand {
	pub hand: Hand,
	pub entity: EntityRef,
	pub position: Isometry3,
	pub velocity: Vec3,
	/// Whether the hand holds anything.
	pub holding: bool,
}

/// What the character knows about the player this frame.
#[derive(Clone, Debug)]
pub struct Player {
	pub head: Isometry3,
	pub head_entity: EntityRef,
	pub velocity: Vec3,
	pub hands: Vec<PlayerHand>,
	/// Height of the floor under the player.
	pub floor: f32,
}

impl Player {
	pub fn hand(&self, hand: Hand) -> Option<&PlayerHand> {
		self.hands.iter().find(|h| h.hand == hand)
	}

	/// Position on the floor under the player's head.
	pub fn floor_pos(&self) -> Point3 {
		let head = self.head.translation.vector;
		point!(head.x, self.floor, head.z)
	}
}

/// Simple eased blend towards a target value over a duration (OpenViva's `Tools.EaseBlend`).
#[derive(Clone, Copy, Debug, Default)]
pub struct EaseBlend {
	pub value: f32,
	start: f32,
	target: f32,
	duration: f32,
	time: f32,
}

impl EaseBlend {
	pub fn new(value: f32) -> Self {
		EaseBlend { value, start: value, target: value, duration: 0.0, time: 0.0 }
	}

	pub fn start(&mut self, target: f32, duration: f32) {
		self.start = self.value;
		self.target = target;
		self.duration = duration;
		self.time = 0.0;
		if duration <= 0.0 {
			self.value = target;
		}
	}

	pub fn reset(&mut self, value: f32) {
		*self = EaseBlend::new(value);
	}

	pub fn update(&mut self, dt: f32) {
		if self.duration <= 0.0 {
			self.value = self.target;
			return;
		}
		self.time = (self.time + dt).min(self.duration);
		let t = self.time / self.duration;
		let eased = if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 };
		self.value = self.start + (self.target - self.start) * eased;
	}

	pub fn target(&self) -> f32 { self.target }

	pub fn finished(&self) -> bool { self.duration <= 0.0 || self.time >= self.duration }
}

/// Per-frame context.
pub struct Ctx<'a> {
	pub app: &'a Application,
	pub dt: f32,
	pub player: Player,
}

/// Runtime state of the character.
pub struct Character {
	pub config: CharacterConfig,
	pub data: Arc<AnimationData>,
	pub voice_data: Arc<VoiceData>,
	pub animator: Animator,
	pub retarget: Retarget,
	pub model: ComponentRef<MMDModel>,
	/// Character's own entities, ignored by ground checks.
	pub own_entities: HashSet<u64>,
	pub bodies: Vec<Body>,
	pub pelvis: usize,
	/// Seconds since the character was created.
	pub time: f32,
	/// -2 (very angry) .. 2 (very happy).
	pub happiness: i32,
	pub tired: bool,

	pub anim: AnimState,
	pub puppet: Puppet,
	pub look: LookAt,
	pub voice: Voice,
	pub active: ActiveTask,
	pub idle: Idle,
	pub headpat: Headpat,
	pub handhold: Handhold,
	pub poke: Poke,
	pub proximity: Proximity,
	pub hug: Hug,
	pub kiss: Kiss,
	pub chopsticks: Chopsticks,
	pub gestures: GestureDetector,
	/// Procedural finger posing (chopsticks and other finger animations).
	pub fingers: FingerAnimator,

	pub pose: Pose,
	anim_transforms: Vec<Isometry3>,
	pub bone_world: Vec<Isometry3>,
	pub bone_skin: Vec<Isometry3>,
	/// Last position and smoothed velocity of the player's hands.
	last_hand_positions: HashMap<u64, (Point3, Vec3)>,
	last_head: Option<Point3>,
	/// Shown in the inspector.
	pub debug_log: Vec<String>,
	/// Scripted test scenario, see `scenario`.
	scenario: Option<scenario::Scenario>,
}

impl Character {
	pub fn new(config: CharacterConfig, model: &MMDModel, entity: &Entity, application: &Application) -> Result<Self> {
		let renderer = &mut *application.renderer.borrow_mut();
		let dir = &config.data;
		let data = renderer.load(JsonAsset::<AnimationData>::at(format!("{dir}/animations.json")))?;
		let voice_data = renderer.load(JsonAsset::<VoiceData>::at(format!("{dir}/voice.json")))?;
		let controller = renderer.load(JsonAsset::<ControllerDesc>::at(format!("{dir}/controller.json")))?;
		let skeleton = renderer.load(JsonAsset::<SkeletonDesc>::at(format!("{dir}/skeleton.json")))?;

		let animator = Animator::new(controller, renderer)?;
		let retarget = Retarget::new(&skeleton, &config.bones, &model.state().bones)?;

		let anchor = *entity.state().position;
		let forward = anchor.rotation * Vec3::z();
		let yaw = forward.x.atan2(forward.z);

		let mut character = Character {
			config,
			data,
			voice_data,
			animator,
			retarget,
			model: model.as_cref(),
			own_entities: HashSet::new(),
			bodies: vec![],
			pelvis: 0,
			time: 0.0,
			happiness: 2,
			tired: false,
			anim: AnimState::new(),
			puppet: Puppet::new(anchor.translation.vector.into(), yaw),
			look: LookAt::new(),
			voice: Voice::new(),
			active: ActiveTask::Idle,
			idle: Idle::new(),
			headpat: Headpat::new(),
			handhold: Handhold::new(),
			poke: Poke::new(),
			proximity: Proximity::new(),
			hug: Hug::new(),
			kiss: Kiss::new(),
			chopsticks: Chopsticks::new(),
			gestures: GestureDetector::new(),
			fingers: FingerAnimator::new(),
			pose: Pose::rest(),
			anim_transforms: vec![],
			bone_world: vec![],
			bone_skin: vec![],
			last_hand_positions: HashMap::new(),
			last_head: None,
			debug_log: vec![],
			scenario: scenario::Scenario::from_env(),
		};

		character.init_animations();

		Ok(character)
	}

	/// Finds the ragdoll bodies once the model created them. Returns `false` if they aren't ready yet.
	pub fn init_bodies(&mut self, entity: &Entity, application: &Application) -> bool {
		let Some(model) = self.model.get(application) else { return false };
		let state = model.state();
		let rigid_bodies: Vec<_> = state.rigid_bodies(application).collect();
		if rigid_bodies.len() <= 1 || rigid_bodies.iter().any(|rb| rb.entity(application).try_state().is_none()) {
			return false;
		}

		let hips = self.retarget.mmd_bone(human::HIPS).unwrap_or(0);
		let mut bodies = vec![];
		for rb in &rigid_bodies {
			let Some(part) = rb.body_part else { continue };
			let rb_entity = rb.entity(application);
			let bone = if part == BodyPart::Pelvis { hips } else { rb.bone };
			bodies.push(Body {
				entity: rb_entity.as_ref(),
				part,
				bone,
				rest: rb.rest_pos.get(),
				parent: None,
				position: *rb_entity.state().position,
				velocity: Vec3::zeros(),
				angular_velocity: Vec3::zeros(),
				grabbed: None,
				target: *rb_entity.state().position,
			});
		}

		// Parent of a body: the body of the closest ancestor bone.
		let bone_of: HashMap<usize, usize> = bodies.iter().enumerate().map(|(id, b)| (b.bone, id)).collect();
		for id in 0..bodies.len() {
			let mut cur = state.bones[bodies[id].bone].parent;
			while let Some(bone) = cur {
				if let Some(&parent) = bone_of.get(&bone) {
					bodies[id].parent = Some(parent);
					break;
				}
				cur = state.bones[bone].parent;
			}
			if bodies[id].parent.is_none() && bodies[id].part != BodyPart::Pelvis {
				bodies[id].parent = bodies.iter().position(|b| b.part == BodyPart::Pelvis);
			}
		}

		self.pelvis = bodies.iter().position(|b| b.part == BodyPart::Pelvis).unwrap_or(0);
		self.own_entities = rigid_bodies.iter().map(|rb| rb.entity(application).id).collect();
		self.own_entities.extend(bodies.iter().filter_map(|b| b.entity.raw()));
		self.own_entities.insert(entity.id);

		{
			let mut physics = application.physics.borrow_mut();
			// Hair, skirt and other free bodies keep their low gravity (they are very heavy in MMD models), but get
			// damped so that they don't whip around when the ragdoll moves fast.
			for rb in &rigid_bodies {
				if rb.body_part.is_none() {
					let body = rb.entity(application).rigid_body_mut(&mut physics);
					body.set_linear_damping(1.0);
					body.set_angular_damping(2.0);
				}
			}
			for body in &bodies {
				if let Some(ent) = body.entity.get(application) {
					ent.set_tag("Character", true);
					let rb = ent.rigid_body_mut(&mut physics);
					rb.set_gravity_scale(1.0, true);
					rb.enable_ccd(true);
				}
			}
		}

		self.bodies = bodies;
		true
	}

	pub fn body(&self, part: BodyPart) -> Option<&Body> {
		self.bodies.iter().find(|b| b.part == part)
	}

	pub fn body_index(&self, part: BodyPart) -> Option<usize> {
		self.bodies.iter().position(|b| b.part == part)
	}

	pub fn is_happy(&self) -> bool { self.happiness >= 0 }

	pub fn shift_happiness(&mut self, amount: i32) {
		self.happiness = (self.happiness + amount).clamp(-2, 2);
	}

	pub fn log(&mut self, msg: impl Into<String>) {
		let msg = msg.into();
		if crate::debug::debug() || self.scenario.is_some() {
			println!("[Miku {:.2}] {}", self.time, msg);
		}
		self.debug_log.push(format!("{:7.2} {}", self.time, msg));
		if self.debug_log.len() > 30 {
			self.debug_log.remove(0);
		}
	}

	pub fn random() -> f32 {
		rand::thread_rng().r#gen::<f32>()
	}

	/// World position of a bone in the animated pose.
	pub fn bone_position(&self, bone: usize) -> Point3 {
		self.puppet.anchor() * self.bone_world.get(bone).map(|t| Point3::from(t.translation.vector)).unwrap_or_else(Point3::origin)
	}

	/// World position of a sphere from the config, following the physical body of its bone if possible.
	pub fn sphere_position(&self, sphere: &SphereConfig, application: &Application) -> Point3 {
		let center = Point3::from(sphere.center);
		let Some(model) = self.model.get(application) else { return center };
		let state = model.state();
		let bone = &state.bones[sphere.bone];
		if let (Some(rb), Some(rb_transform)) = (bone.rigid_body.get(application), bone.rigid_body_transform) {
			let rb_pos = *rb.state().position;
			let bone_pos = rb_pos * rb_transform.inverse();
			return bone_pos * (center - bone.origin().coords);
		}
		self.puppet.anchor() * self.bone_skin.get(sphere.bone).map_or(center, |skin| skin * center)
	}

	/// Physical head position (or the animated one if unavailable).
	pub fn head_position(&self) -> Point3 {
		self.body(BodyPart::Head).map(|b| Point3::from(b.position.translation.vector))
		    .unwrap_or_else(|| self.bone_position(self.retarget.mmd_bone(human::HEAD).unwrap_or(0)))
	}

	/// Character's facing direction (horizontal).
	pub fn forward(&self) -> Vec3 {
		self.puppet.anchor().rotation * (self.retarget.frame * Vec3::z())
	}

	/// Character's left direction (horizontal).
	pub fn left(&self) -> Vec3 {
		self.puppet.anchor().rotation * (self.retarget.frame * Vec3::x())
	}

	/// Floor position of the character.
	pub fn floor_pos(&self) -> Point3 {
		Point3::from(self.puppet.anchor().translation.vector)
	}

	/// Signed horizontal angle (degrees) from the character's facing to `point`, positive to the right.
	pub fn bearing(&self, from: Point3, forward: Vec3, point: Point3) -> f32 {
		let diff = point - from;
		let forward_angle = forward.x.atan2(forward.z);
		let target_angle = diff.x.atan2(diff.z);
		let mut angle = (forward_angle - target_angle).to_degrees();
		while angle > 180.0 { angle -= 360.0; }
		while angle < -180.0 { angle += 360.0; }
		angle * self.handedness()
	}

	/// +1 if the model's left is to the left of its forward in world space (right handed), -1 if mirrored.
	fn handedness(&self) -> f32 {
		let left = self.left();
		let forward = self.forward();
		if forward.cross(&left).y >= 0.0 { 1.0 } else { -1.0 }
	}

	/// Gathers the player's head and hands.
	fn read_player(&mut self, application: &Application, dt: f32) -> Player {
		let head_entity = application.pov.clone();
		let head = head_entity.get(application).map(|e| *e.state().position).unwrap_or_else(Isometry3::identity);
		let head_pos = Point3::from(head.translation.vector);
		let velocity = self.last_head.map_or(Vec3::zeros(), |last| (head_pos - last) / dt.max(0.0001));
		self.last_head = Some(head_pos);

		let mut hands = vec![];
		for entity in application.find_all_entities(|e| e.find_component_by_type::<HandComponent>().is_some()) {
			let Some(component) = entity.find_component_by_type::<HandComponent>() else { continue };
			let position = *entity.state().position;
			let pos = Point3::from(position.translation.vector);
			// Smoothed over ~0.03s, like a physics velocity, so that tracking jitter doesn't count as fast movement.
			let velocity = self.last_hand_positions.get(&entity.id).map_or(Vec3::zeros(), |(last, velocity)| {
				let raw = (pos - last) / dt.max(0.0001);
				velocity.lerp(&raw, (dt / 0.03).min(1.0))
			});
			self.last_hand_positions.insert(entity.id, (pos, velocity));
			hands.push(PlayerHand {
				hand: component.hand,
				entity: entity.as_ref(),
				position,
				velocity,
				holding: component.grabbed_entity().raw().is_some(),
			});
		}

		let floor = application.find_entity(|e| e.has_tag("World"))
		                       .map(|e| e.state().position.translation.vector.y)
		                       .unwrap_or(0.0);

		Player { head, head_entity, velocity, hands, floor }
	}

	/// Reads the simulated state of the bodies.
	fn read_bodies(&mut self, application: &Application) {
		for body in &mut self.bodies {
			if let Some(entity) = body.entity.get(application) {
				let state = entity.state();
				body.position = *state.position;
				body.velocity = *state.velocity;
				body.angular_velocity = *state.angular_velocity;
				body.grabbed = entity.tag::<ComponentRef<HandComponent>>("Grabbed")
				                     .and_then(|hand| hand.get(application))
				                     .map(|hand| hand.hand);
			}
		}
	}

	/// Runs one frame of the character.
	pub fn update(&mut self, application: &Application, dt: f32) {
		if dt <= 0.0 { return; }
		self.time += dt;

		let player = self.read_player(application, dt);
		self.read_bodies(application);
		let ctx = Ctx { app: application, dt, player };
		
		if let Some(mut scenario) = self.scenario.take() {
			self.run_scenario(&ctx, &mut scenario);
			self.scenario = Some(scenario);
		}

		self.update_gestures(&ctx);
		self.update_balance(&ctx);
		self.update_grabs(&ctx);

		self.update_headpat(&ctx);
		self.update_handhold(&ctx);
		self.update_poke(&ctx);
		self.update_proximity(&ctx);
		self.update_hug(&ctx);
		self.update_kiss(&ctx);
		self.update_active(&ctx);

		self.update_animation_graph(&ctx);
		self.update_locomotion(&ctx);
		self.update_voice(&ctx);

		let anim_dt = dt * self.anim.speed;
		self.animator.update(anim_dt);
		let mut pose = std::mem::replace(&mut self.pose, Pose::rest());
		self.animator.evaluate(&mut pose);
		self.update_look_at(&ctx, &mut pose);
		self.apply_procedural(&ctx, &mut pose);
		self.pose = pose;

		self.retarget.apply(&self.pose, &mut self.anim_transforms);
		self.retarget.forward_kinematics(&self.anim_transforms, &mut self.bone_world, &mut self.bone_skin);

		self.update_anchor(&ctx);
		self.apply_to_model(application);
		self.drive_bodies(&ctx);
		self.draw_debug(&ctx);
	}

	/// Writes bone animations and morphs into the MMD model. Bones following ragdoll bodies are
	/// overridden by the physics.
	fn apply_to_model(&mut self, application: &Application) {
		let Some(model) = self.model.get(application) else { return };
		let mut state = model.state_mut();

		for (bone, anim) in state.bones.iter_mut().zip(&self.anim_transforms) {
			bone.anim_transform = nalgebra::Similarity3::from_isometry(*anim, 1.0);
		}

		// The source face rig often activates several eye-closing blend shapes at once. MMD's eye
		// morphs are full shape replacements, so adding them all makes the eyelids collapse into
		// the cheeks. Give the combined "happy closed eyes" morph priority and collapse each
		// side's upper/lower lid shapes into one closure.
		let morph_value = |name: &str| human::find_morph(name).map_or(0.0, |id| self.pose.morphs[id]);
		let happy_closed = morph_value("happyClosedEyelids").clamp(0.0, 1.0);
		let upper_l = morph_value("upperEyelid_l").clamp(0.0, 1.0);
		let upper_r = morph_value("upperEyelid_r").clamp(0.0, 1.0);

		for (name, targets) in &self.config.morphs {
			let Some(id) = human::find_morph(name) else { continue };
			let mut value = self.pose.morphs[id];
			match name.as_str() {
				"upperEyelid_l" | "upperEyelid_r" | "madEye_l" | "madEye_r" => {
					value *= 1.0 - happy_closed;
				},
				"lowerEyelid_l" => value *= (1.0 - happy_closed) * (1.0 - upper_l),
				"lowerEyelid_r" => value *= (1.0 - happy_closed) * (1.0 - upper_r),
				_ => {},
			}
			for &(morph, weight) in targets {
				if let Some(slot) = state.morphs.get_mut(morph) {
					*slot = (*slot + value * weight).min(1.0);
				}
			}
		}
	}

	fn draw_debug(&self, ctx: &Ctx) {
		use crate::debug;
		use crate::math::Color;

		if !debug::get_flag_or_default::<bool>("DebugCharacterDraw") { return; }

		for body in &self.bodies {
			let pos = Point3::from(body.position.translation.vector);
			let target = Point3::from(body.target.translation.vector);
			debug::draw_line(pos, target, 3.0, Color::YELLOW);
			debug::draw_point(target, 8.0, Color::YELLOW);
		}

		let floor = self.floor_pos();
		debug::draw_line(floor, floor + self.forward() * 0.5, 4.0, Color::CYAN);

		let head = self.sphere_position(&self.config.headpat, ctx.app);
		debug::draw_sphere(Isometry3::from_parts(head.coords.into(), Rot3::identity()), self.config.headpat.radius, Color::GREEN.opactiy(0.2), Color::GREEN);
		let face = self.sphere_position(&self.config.face, ctx.app);
		debug::draw_sphere(Isometry3::from_parts(face.coords.into(), Rot3::identity()), self.config.face.radius, Color::MAGENTA.opactiy(0.2), Color::MAGENTA);
		let tummy = self.sphere_position(&self.config.tummy, ctx.app);
		debug::draw_sphere(Isometry3::from_parts(tummy.coords.into(), Rot3::identity()), self.config.tummy.radius, Color::BLUE.opactiy(0.2), Color::BLUE);

		debug::draw_text(format!("{} [{}] {}", self.anim.current, self.anim.body_state, self.active.name()),
		                 floor + Vec3::y() * 1.8, debug::DebugOffset::center(0.0, 0.0), 32.0, Color::WHITE);
	}
}
