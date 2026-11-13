use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use serde_derive::Deserialize;

use crate::renderer::Renderer;
use super::human;
use super::clip::{Clip, ClipAsset};
use super::pose::{Pose, PoseAccumulator};


/// Animator controller exported from Unity by `tools/openviva/import.py` (`controller.json`).
#[derive(Deserialize, Debug)]
pub struct ControllerDesc {
	pub parameters: Vec<String>,
	pub layers: Vec<LayerDesc>,
	pub clips: HashMap<String, ClipDesc>,
}

#[derive(Deserialize, Debug)]
pub struct LayerDesc {
	pub name: String,
	pub states: HashMap<String, StateDesc>,
}

#[derive(Deserialize, Debug)]
pub struct StateDesc {
	pub motion: MotionDesc,
	pub speed: f32,
	pub speed_parameter: Option<String>,
	/// Parameter driving the normalized time of the state, instead of playing it.
	pub time_parameter: Option<String>,
	pub mirror: bool,
	#[allow(dead_code)]
	pub cycle_offset: f32,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum MotionDesc {
	Clip(String),
	Tree(TreeDesc),
}

/// Unity blend tree. `blend_type`: 0 = 1D, 1/2/3 = 2D (simple directional, freeform directional, freeform
/// cartesian), 4 = direct.
#[derive(Deserialize, Debug)]
pub struct TreeDesc {
	pub blend_type: u32,
	pub parameter: String,
	pub parameter_y: String,
	pub children: Vec<TreeChildDesc>,
}

#[derive(Deserialize, Debug)]
pub struct TreeChildDesc {
	pub motion: MotionDesc,
	pub threshold: f32,
	pub position: [f32; 2],
	pub time_scale: f32,
	pub mirror: bool,
	pub direct_parameter: String,
}

#[derive(Deserialize, Debug)]
pub struct ClipDesc {
	pub file: String,
	#[allow(dead_code)]
	pub length: f32,
	#[allow(dead_code)]
	pub looping: bool,
}

/// Layers of the controller, in order of evaluation.
pub const LAYER_LEGS: usize = 0;
pub const LAYER_TORSO: usize = 1;
pub const LAYER_FACE: usize = 2;

/// A state playing on a layer.
#[derive(Clone, Debug)]
struct PlayingState {
	name: String,
	/// Normalized time; 1.0 = end of the first loop.
	time: f32,
}

#[derive(Clone, Debug, Default)]
struct LayerState {
	current: Option<PlayingState>,
	/// State being cross-faded to, with the elapsed and total transition time.
	next: Option<(PlayingState, f32, f32)>,
}

/// One weighted clip of an evaluated motion.
struct WeightedClip<'a> {
	clip: &'a Arc<Clip>,
	weight: f32,
	mirror: bool,
	time_scale: f32,
}

/// Plays the states of an animator controller, similarly to Unity's `Animator`: each layer plays one
/// state (with cross-fades), states play clips or blend trees driven by float parameters.
pub struct Animator {
	desc: Arc<ControllerDesc>,
	clips: HashMap<String, Arc<Clip>>,
	pub params: HashMap<String, f32>,
	layers: Vec<LayerState>,
	scratch: Pose,
	scratch_layer: Pose,
}

impl Animator {
	pub fn new(desc: Arc<ControllerDesc>, renderer: &mut Renderer) -> Result<Self> {
		let mut clips = HashMap::new();
		for (name, clip) in &desc.clips {
			clips.insert(name.clone(), renderer.load(ClipAsset::at(&clip.file))?);
		}

		let params = desc.parameters.iter().map(|name| (name.clone(), 0.0)).collect();
		let layers = vec![LayerState::default(); desc.layers.len()];

		Ok(Animator {
			desc,
			clips,
			params,
			layers,
			scratch: Pose::rest(),
			scratch_layer: Pose::rest(),
		})
	}

	pub fn has_state(&self, layer: usize, state: &str) -> bool {
		self.desc.layers.get(layer).map_or(false, |l| l.states.contains_key(state))
	}

	pub fn param(&self, name: &str) -> f32 {
		self.params.get(name).copied().unwrap_or(0.0)
	}

	pub fn set_param(&mut self, name: &str, value: f32) {
		self.params.insert(name.to_string(), value);
	}

	/// Cross-fades a layer to `state` over `duration` seconds, starting at normalized `time_offset`.
	/// Unknown states are ignored.
	pub fn crossfade(&mut self, layer: usize, state: &str, duration: f32, time_offset: f32) {
		if !self.has_state(layer, state) { return; }
		let layer_state = &mut self.layers[layer];

		// Interrupting a transition: settle on whichever state dominates.
		if let Some((next, elapsed, total)) = layer_state.next.take() {
			if total <= 0.0 || elapsed / total > 0.5 {
				layer_state.current = Some(next);
			}
		}

		let playing = PlayingState { name: state.to_string(), time: time_offset };

		if layer_state.current.is_none() || duration <= 0.0 {
			layer_state.current = Some(playing);
		} else {
			layer_state.next = Some((playing, 0.0, duration));
		}
	}

	/// Name of the state the layer is playing or transitioning to.
	pub fn state(&self, layer: usize) -> Option<&str> {
		let layer = self.layers.get(layer)?;
		layer.next.as_ref().map(|n| &n.0).or(layer.current.as_ref()).map(|s| s.name.as_str())
	}

	/// Normalized time of the state the layer is playing or transitioning to (like `GetLayerAnimNormTime`).
	pub fn normalized_time(&self, layer: usize) -> f32 {
		let Some(layer) = self.layers.get(layer) else { return 0.0 };
		layer.next.as_ref().map(|n| &n.0).or(layer.current.as_ref()).map_or(0.0, |s| s.time)
	}

	/// Length in seconds of the state the layer is playing or transitioning to.
	pub fn length(&self, layer: usize) -> f32 {
		self.state(layer).map_or(1.0, |state| self.state_length(layer, state))
	}

	pub fn in_transition(&self, layer: usize) -> bool {
		self.layers.get(layer).map_or(false, |l| l.next.is_some())
	}

	fn state_length(&self, layer: usize, state: &str) -> f32 {
		let Some(desc) = self.desc.layers[layer].states.get(state) else { return 1.0 };
		let mut weights = vec![];
		self.motion_weights(&desc.motion, 1.0, false, 1.0, &mut weights);
		let length: f32 = weights.iter().map(|w| w.weight * w.clip.length() / w.time_scale.max(0.01)).sum();
		length.max(0.01)
	}

	fn state_speed(&self, layer: usize, state: &str) -> f32 {
		let Some(desc) = self.desc.layers[layer].states.get(state) else { return 1.0 };
		desc.speed * desc.speed_parameter.as_ref().map_or(1.0, |param| self.param(param))
	}

	fn advance(&self, layer: usize, state: &mut PlayingState, dt: f32) {
		let Some(desc) = self.desc.layers[layer].states.get(&state.name) else { return };
		if let Some(param) = &desc.time_parameter {
			state.time = self.param(param);
		} else {
			state.time += dt * self.state_speed(layer, &state.name) / self.state_length(layer, &state.name);
		}
	}

	/// Advances all layers by `dt` seconds.
	pub fn update(&mut self, dt: f32) {
		for layer in 0..self.layers.len() {
			let mut layer_state = std::mem::take(&mut self.layers[layer]);

			if let Some(current) = &mut layer_state.current {
				self.advance(layer, current, dt);
			}

			if let Some((next, elapsed, total)) = &mut layer_state.next {
				self.advance(layer, next, dt);
				*elapsed += dt;
				if *elapsed >= *total {
					layer_state.current = layer_state.next.take().map(|n| n.0);
				}
			}

			self.layers[layer] = layer_state;
		}
	}

	/// Evaluates all layers into `out`: the legs layer drives the whole body, the torso layer overrides the
	/// upper body and the face layer overrides blend shapes and eyes.
	pub fn evaluate(&mut self, out: &mut Pose) {
		*out = Pose::rest();

		for layer in 0..self.layers.len() {
			let mut pose = std::mem::replace(&mut self.scratch_layer, Pose::rest());
			if !self.evaluate_layer(layer, &mut pose) {
				self.scratch_layer = pose;
				continue;
			}

			match layer {
				LAYER_LEGS => out.clone_from(&pose),
				LAYER_TORSO => out.blend(&pose, 1.0, human::is_upper_body, false),
				LAYER_FACE => out.blend(&pose, 1.0, human::is_eye, true),
				_ => {},
			}

			self.scratch_layer = pose;
		}
	}

	fn evaluate_layer(&mut self, layer: usize, out: &mut Pose) -> bool {
		let layer_state = self.layers[layer].clone();
		let Some(current) = &layer_state.current else { return false };

		self.evaluate_state(layer, current, out);

		if let Some((next, elapsed, total)) = &layer_state.next {
			let mut next_pose = std::mem::replace(&mut self.scratch, Pose::rest());
			self.evaluate_state(layer, next, &mut next_pose);
			let weight = if *total > 0.0 { (elapsed / total).clamp(0.0, 1.0) } else { 1.0 };
			out.blend(&next_pose, weight, |_| true, true);
			self.scratch = next_pose;
		}

		true
	}

	fn evaluate_state(&mut self, layer: usize, state: &PlayingState, out: &mut Pose) {
		let Some(desc) = self.desc.layers[layer].states.get(&state.name) else { return };

		let mut weights = vec![];
		self.motion_weights(&desc.motion, 1.0, desc.mirror, 1.0, &mut weights);

		let mut accumulator = PoseAccumulator::new();
		let mut sample = Pose::rest();
		for weighted in &weights {
			if weighted.weight <= 0.0001 { continue; }
			let clip = weighted.clip;
			let time = if clip.looping { state.time.rem_euclid(1.0) } else { state.time.clamp(0.0, 1.0) } * clip.length();
			clip.sample(time, &mut sample);
			if weighted.mirror {
				sample.mirror();
			}
			accumulator.add(&sample, weighted.weight);
		}
		accumulator.finish(out);
	}

	fn motion_weights<'a>(&'a self, motion: &MotionDesc, weight: f32, mirror: bool, time_scale: f32, out: &mut Vec<WeightedClip<'a>>) {
		match motion {
			MotionDesc::Clip(name) => {
				if let Some(clip) = self.clips.get(name) {
					out.push(WeightedClip { clip, weight, mirror, time_scale });
				}
			},
			MotionDesc::Tree(tree) => {
				let weights = self.tree_weights(tree);
				for (child, child_weight) in tree.children.iter().zip(weights) {
					if child_weight > 0.0 {
						self.motion_weights(&child.motion, weight * child_weight, mirror ^ child.mirror, time_scale * child.time_scale, out);
					}
				}
			},
		}
	}

	fn tree_weights(&self, tree: &TreeDesc) -> Vec<f32> {
		let count = tree.children.len();
		let mut weights = vec![0.0; count];
		if count == 0 { return weights; }

		match tree.blend_type {
			0 => {
				let value = self.param(&tree.parameter);
				let mut order: Vec<usize> = (0..count).collect();
				order.sort_by(|a, b| tree.children[*a].threshold.total_cmp(&tree.children[*b].threshold));
				let first = order[0];
				let last = order[count - 1];
				if value <= tree.children[first].threshold {
					weights[first] = 1.0;
				} else if value >= tree.children[last].threshold {
					weights[last] = 1.0;
				} else {
					for pair in order.windows(2) {
						let (a, b) = (tree.children[pair[0]].threshold, tree.children[pair[1]].threshold);
						if value >= a && value <= b {
							let t = if b - a > f32::EPSILON { (value - a) / (b - a) } else { 0.0 };
							weights[pair[0]] = 1.0 - t;
							weights[pair[1]] = t;
							break;
						}
					}
				}
			},
			4 => {
				for (weight, child) in weights.iter_mut().zip(&tree.children) {
					*weight = self.param(&child.direct_parameter);
				}
			},
			_ => {
				// Gradient band interpolation (cartesian), used for all 2D blend types.
				let p = vector!(self.param(&tree.parameter), self.param(&tree.parameter_y));
				let positions: Vec<_> = tree.children.iter().map(|c| vector!(c.position[0], c.position[1])).collect();
				for i in 0..count {
					let mut weight: f32 = 1.0;
					for j in 0..count {
						if i == j { continue; }
						let pij = positions[j] - positions[i];
						let len = pij.norm_squared();
						if len <= f32::EPSILON { continue; }
						let w = 1.0 - (p - positions[i]).dot(&pij) / len;
						weight = weight.min(w.clamp(0.0, 1.0));
					}
					weights[i] = weight;
				}
				let sum: f32 = weights.iter().sum();
				if sum > f32::EPSILON {
					weights.iter_mut().for_each(|w| *w /= sum);
				} else {
					let closest = (0..count).min_by(|a, b| (positions[*a] - p).norm().total_cmp(&(positions[*b] - p).norm())).unwrap();
					weights[closest] = 1.0;
				}
			},
		}

		weights
	}
}
