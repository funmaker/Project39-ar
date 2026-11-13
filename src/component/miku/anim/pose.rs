use nalgebra::{Quaternion, Vector4};

use crate::math::{Rot3, Vec3};
use super::human;


/// Pose of the humanoid skeleton: local bone rotations (identity = rest pose), hips offset from
/// the rest position (in hips heights) and blend shape weights.
#[derive(Clone, Debug)]
pub struct Pose {
	pub rotations: Vec<Rot3>,
	pub root: Vec3,
	pub morphs: Vec<f32>,
}

impl Pose {
	pub fn rest() -> Self {
		Pose {
			rotations: vec![Rot3::identity(); human::count()],
			root: Vec3::zeros(),
			morphs: vec![0.0; human::MORPHS.len()],
		}
	}

	/// Mirrors the pose left to right.
	pub fn mirror(&mut self) {
		let rotations = self.rotations.clone();
		for (id, bone) in human::BONES.iter().enumerate() {
			self.rotations[id] = mirror_rotation(rotations[bone.mirror]);
		}
		self.root.x = -self.root.x;
		let morphs = self.morphs.clone();
		for id in 0..morphs.len() {
			self.morphs[id] = morphs[human::mirror_morph(id)];
		}
	}

	/// Blends `other` into this pose, `weight` 0 keeps this pose, 1 replaces it.
	pub fn blend(&mut self, other: &Pose, weight: f32, bones: impl Fn(usize) -> bool, morphs: bool) {
		if weight <= 0.0 { return; }
		for id in 0..self.rotations.len() {
			if bones(id) {
				self.rotations[id] = nlerp(self.rotations[id], other.rotations[id], weight);
			}
		}
		if bones(human::HIPS) {
			self.root = self.root.lerp(&other.root, weight);
		}
		if morphs {
			for (morph, other) in self.morphs.iter_mut().zip(&other.morphs) {
				*morph += (other - *morph) * weight;
			}
		}
	}

	/// Computes the humanoid space rotation of every bone (forward kinematics).
	pub fn global_rotations(&self, out: &mut Vec<Rot3>) {
		out.clear();
		for (id, bone) in human::BONES.iter().enumerate() {
			let global = match bone.parent {
				Some(parent) => out[parent] * self.rotations[id],
				None => self.rotations[id],
			};
			out.push(global);
		}
	}
}

/// Mirrors a humanoid space rotation along the X (left-right) axis.
pub fn mirror_rotation(rot: Rot3) -> Rot3 {
	let q = rot.quaternion();
	Rot3::new_unchecked(Quaternion::new(q.w, q.i, -q.j, -q.k))
}

/// Normalized linear interpolation of rotations, taking the shortest path.
pub fn nlerp(from: Rot3, to: Rot3, weight: f32) -> Rot3 {
	let a = from.quaternion().coords;
	let mut b = to.quaternion().coords;
	if a.dot(&b) < 0.0 {
		b = -b;
	}
	Rot3::new_normalize(Quaternion::from(a.lerp(&b, weight)))
}

/// Accumulates a weighted average of poses.
pub struct PoseAccumulator {
	rotations: Vec<Vector4<f32>>,
	root: Vec3,
	morphs: Vec<f32>,
	weight: f32,
}

impl PoseAccumulator {
	pub fn new() -> Self {
		PoseAccumulator {
			rotations: vec![Vector4::zeros(); human::count()],
			root: Vec3::zeros(),
			morphs: vec![0.0; human::MORPHS.len()],
			weight: 0.0,
		}
	}

	pub fn clear(&mut self) {
		self.rotations.fill(Vector4::zeros());
		self.root = Vec3::zeros();
		self.morphs.fill(0.0);
		self.weight = 0.0;
	}

	pub fn add(&mut self, pose: &Pose, weight: f32) {
		for (acc, rot) in self.rotations.iter_mut().zip(&pose.rotations) {
			let q = rot.quaternion().coords;
			if acc.dot(&q) < 0.0 {
				*acc -= q * weight;
			} else {
				*acc += q * weight;
			}
		}
		self.root += pose.root * weight;
		for (acc, morph) in self.morphs.iter_mut().zip(&pose.morphs) {
			*acc += morph * weight;
		}
		self.weight += weight;
	}

	/// Writes the average into `out`. Returns `false` (leaving `out` intact) if nothing was added.
	pub fn finish(&self, out: &mut Pose) -> bool {
		if self.weight <= f32::EPSILON {
			return false;
		}
		for (rot, acc) in out.rotations.iter_mut().zip(&self.rotations) {
			*rot = if acc.norm_squared() > f32::EPSILON {
				Rot3::new_normalize(Quaternion::from(*acc))
			} else {
				Rot3::identity()
			};
		}
		out.root = self.root / self.weight;
		for (morph, acc) in out.morphs.iter_mut().zip(&self.morphs) {
			*morph = acc / self.weight;
		}
		true
	}
}
