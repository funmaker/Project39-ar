
//! Data files imported from OpenViva by `tools/openviva/import.py`.

use std::collections::HashMap;
use serde::Deserializer;
use serde_derive::Deserialize;


/// Animation manifest (`animations.json`): every animation the character can play, as defined in
/// OpenViva's `Lily.AnimationManifest.cs`, with the body state graph.
#[derive(Deserialize, Debug)]
pub struct AnimationData {
	pub animations: HashMap<String, AnimInfo>,
	pub body_states: HashMap<String, BodyStateInfo>,
	/// Order of OpenViva's `Lily.Animation` enum. Some logic picks variants by offsetting the enum.
	pub enum_order: Vec<String>,
}

impl AnimationData {
	/// Returns the animation `offset` entries after `base` in the enum, if it exists and is imported.
	pub fn offset(&self, base: &str, offset: usize) -> Option<&str> {
		let index = self.enum_order.iter().position(|name| name == base)?;
		let name = self.enum_order.get(index + offset)?;
		self.animations.contains_key(name).then_some(name.as_str())
	}
}

#[derive(Deserialize, Debug)]
pub struct AnimInfo {
	pub transition: Transition,
	/// State of the torso animator layer.
	pub torso: String,
	/// State of the legs animator layer.
	pub legs: String,
	/// State of the face animator layer.
	pub face: String,
	#[serde(deserialize_with = "priority")]
	pub priority: i32,
	/// Body state required to play the animation.
	pub condition: String,
	/// Body state after the animation ends.
	pub new_state: String,
	pub transition_time: f32,
	pub logic: AnimLogic,
	pub events: Vec<AnimEvent>,
	pub flags: Vec<String>,
	/// Float parameter syncing the torso state's time with the legs state.
	#[serde(default)]
	pub torso_sync: Option<String>,
}

impl AnimInfo {
	pub fn is_idle(&self) -> bool {
		self.flags.iter().any(|f| f == "IDLE_STATE")
	}

	pub fn disables_ragdoll_check(&self) -> bool {
		self.flags.iter().any(|f| f == "DISABLE_RAGDOLL_CHECK")
	}
}

/// What to play once an animation ends.
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Transition {
	/// Return to the body state's idle animation.
	Idle,
	/// Play the next animation.
	Next { next: String },
	/// Keep looping until another animation is requested.
	Loop,
	/// Behavior specific transition.
	Custom { handler: String },
}

/// Look-at behavior of an animation. See `LookAt`.
#[derive(Deserialize, Debug, Clone, Copy)]
pub struct AnimLogic {
	pub body_flags: u32,
	pub body_flag_percent: f32,
	pub body_flag_duration: f32,
	pub eye_flags: u32,
	pub eye_flag_duration: f32,
}

/// Event fired at a normalized time of an animation.
#[derive(Deserialize, Debug, Clone)]
pub struct AnimEvent {
	pub time: f32,
	pub name: String,
	pub values: Vec<serde_json::Value>,
}

impl AnimEvent {
	pub fn str(&self, index: usize) -> Option<&str> {
		self.values.get(index).and_then(|v| v.as_str())
	}

	pub fn num(&self, index: usize) -> f32 {
		self.values.get(index).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32
	}
}

#[derive(Deserialize, Debug)]
pub struct BodyStateInfo {
	pub check_balance: bool,
	/// Named animation sets (eg. `IDLE_HAPPY`), one of which is picked at random.
	pub sets: HashMap<String, Vec<String>>,
	/// Animations transitioning to other body states.
	pub connections: HashMap<String, String>,
	#[allow(dead_code)]
	pub properties: HashMap<String, f32>,
}

/// Voice lines and sound effects (`voice.json`).
#[derive(Deserialize, Debug)]
pub struct VoiceData {
	pub lines: HashMap<String, Vec<String>>,
	pub sfx: HashMap<String, Vec<String>>,
}

pub const PRIORITY_NONE: i32 = -1;

fn priority<'de, D: Deserializer<'de>>(de: D) -> Result<i32, D::Error> {
	Ok(match <String as serde::Deserialize>::deserialize(de)?.as_str() {
		"LOW" => 0,
		"MEDIUM" => 1,
		"HIGH" => 2,
		"VERY_HIGH" => 3,
		_ => PRIORITY_NONE,
	})
}
