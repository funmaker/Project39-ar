use std::collections::HashMap;
use anyhow::Result;
use serde_derive::Deserialize;
use thiserror::Error;

use crate::math::{Isometry3, Mat3, Rot3, Translation3, Vec3};
use super::super::super::model::mmd::MMDBone;
use super::human;
use super::pose::Pose;


/// Rest pose of the source rig in the humanoid space (`skeleton.json`).
#[derive(Deserialize, Debug)]
pub struct SkeletonDesc {
	pub bones: Vec<SkeletonBoneDesc>,
}

#[derive(Deserialize, Debug)]
pub struct SkeletonBoneDesc {
	pub name: String,
	/// Direction the bone points to in the rest pose.
	pub direction: Option<[f32; 3]>,
	/// Rest position, in hips heights.
	#[allow(dead_code)]
	pub position: [f32; 3],
}

#[derive(Clone, Debug)]
struct TargetBone {
	/// MMD bone driven by the humanoid bone.
	mmd: usize,
	/// Model space rotation aligning the target rest pose with the source rest pose.
	align: Rot3,
}

/// Maps humanoid poses onto an MMD model's bones.
///
/// For every humanoid bone, the model space rotation of its MMD bone is the humanoid space rotation
/// (converted to model space) times an alignment rotation that turns the MMD bone's rest direction into
/// the source rig's rest direction. MMD bones have no rest rotations, so local rotations follow directly.
pub struct Retarget {
	/// Humanoid space to model space rotation.
	pub frame: Rot3,
	bones: Vec<Option<TargetBone>>,
	/// Humanoid bone of each MMD bone, if mapped.
	mmd_to_human: Vec<Option<usize>>,
	/// Closest mapped ancestor of each MMD bone.
	mmd_mapped_parent: Vec<Option<usize>>,
	mmd_parents: Vec<Option<usize>>,
	mmd_local: Vec<Translation3>,
	mmd_rest: Vec<Translation3>,
	/// Height of the hips above the model's origin, scales the hips motion.
	pub hips_height: f32,
	globals: Vec<Rot3>,
	model_globals: Vec<Option<Rot3>>,
}

impl Retarget {
	/// `mapping` maps humanoid bone names to MMD bone ids.
	pub fn new(skeleton: &SkeletonDesc, mapping: &HashMap<String, usize>, mmd_bones: &[MMDBone]) -> Result<Self> {
		let mut bones: Vec<Option<TargetBone>> = vec![None; human::count()];
		let mut mmd_to_human = vec![None; mmd_bones.len()];

		for (name, &mmd) in mapping {
			let id = human::find(name).ok_or_else(|| RetargetError::UnknownBone(name.clone()))?;
			if mmd >= mmd_bones.len() { return Err(RetargetError::NoSuchMMDBone(mmd).into()); }
			bones[id] = Some(TargetBone { mmd, align: Rot3::identity() });
			mmd_to_human[mmd] = Some(id);
		}

		let positions: Vec<Option<Vec3>> = bones.iter().map(|b| b.as_ref().map(|b| mmd_bones[b.mmd].origin().coords)).collect();
		let position = |id: usize| positions[id];
		let required = |id: usize| position(id).ok_or_else(|| RetargetError::MissingBone(human::BONES[id].name.clone()));

		let hips = required(human::HIPS)?;
		let up = (required(human::HEAD)? - hips).normalize();
		let mut left = required(human::Side::Left.upper_arm())? - required(human::Side::Right.upper_arm())?;
		left = (left - up * left.dot(&up)).normalize();
		let forward = left.cross(&up);
		let frame = Rot3::from_matrix(&Mat3::from_columns(&[left, up, forward]));
		let hips_height = hips.dot(&up);

		let source_dirs: HashMap<&str, Vec3> = skeleton.bones.iter()
		                                               .filter_map(|b| b.direction.map(|d| (b.name.as_str(), Vec3::from(d))))
		                                               .collect();

		for (id, bone) in human::BONES.iter().enumerate() {
			let parent_align = bone.parent.and_then(|p| bones[p].as_ref().map(|b| b.align)).unwrap_or(Rot3::identity());
			let Some(target) = &bones[id] else { continue };
			let mmd = target.mmd;

			// Hips, spine, neck, head and eyes keep their orientation; limbs are aligned.
			let aligned = id > human::HEAD && !human::is_eye(id);
			let align = if !aligned {
				Rot3::identity()
			} else if let (Some(tip), Some(source_dir)) = (bone.tip.and_then(|tip| position(tip)), source_dirs.get(bone.name.as_str())) {
				let target_dir = tip - mmd_bones[mmd].origin().coords;
				if target_dir.norm() < 0.0001 {
					parent_align
				} else {
					Rot3::rotation_between(&target_dir, &(frame * source_dir)).unwrap_or(Rot3::identity())
				}
			} else {
				parent_align
			};

			bones[id].as_mut().unwrap().align = align;
		}

		let mmd_parents: Vec<_> = mmd_bones.iter().map(|b| b.parent).collect();
		let mmd_mapped_parent = (0..mmd_bones.len()).map(|id| {
			let mut cur = mmd_parents[id];
			while let Some(parent) = cur {
				if mmd_to_human[parent].is_some() { return Some(parent); }
				cur = mmd_parents[parent];
			}
			None
		}).collect();

		Ok(Retarget {
			frame,
			bones,
			mmd_to_human,
			mmd_mapped_parent,
			mmd_parents,
			mmd_local: mmd_bones.iter().map(|b| b.local_transform).collect(),
			mmd_rest: mmd_bones.iter().map(|b| b.model_transform).collect(),
			hips_height,
			globals: vec![],
			model_globals: vec![None; human::count()],
		})
	}

	/// Rest position of an MMD bone in model space.
	pub fn rest_position(&self, mmd: usize) -> Vec3 {
		self.mmd_rest.get(mmd).map_or(Vec3::zeros(), |t| t.vector)
	}
	
	/// MMD bone driven by a humanoid bone.
	pub fn mmd_bone(&self, human: usize) -> Option<usize> {
		self.bones[human].as_ref().map(|b| b.mmd)
	}

	/// Computes the local animation transform of every MMD bone for `pose`. Unmapped bones get identities.
	pub fn apply(&mut self, pose: &Pose, anim: &mut Vec<Isometry3>) {
		pose.global_rotations(&mut self.globals);

		let frame_inv = self.frame.inverse();
		for (id, target) in self.bones.iter().enumerate() {
			self.model_globals[id] = target.as_ref().map(|t| self.frame * self.globals[id] * frame_inv * t.align);
		}

		anim.clear();
		anim.resize(self.mmd_to_human.len(), Isometry3::identity());

		for (mmd, human) in self.mmd_to_human.iter().enumerate() {
			let Some(human) = *human else { continue };
			let global = self.model_globals[human].unwrap();
			let parent_global = self.mmd_mapped_parent[mmd]
			                        .and_then(|p| self.mmd_to_human[p])
			                        .and_then(|h| self.model_globals[h])
			                        .unwrap_or(Rot3::identity());
			let mut local = Isometry3::from_parts(Translation3::identity(), parent_global.inverse() * global);
			if human == human::HIPS {
				local.translation.vector = self.frame * pose.root * self.hips_height;
			}
			anim[mmd] = local;
		}
	}

	/// Computes model space transforms of all MMD bones (`world`) and their skinning transforms (`skin`,
	/// mapping the rest pose to the animated pose) from local animation transforms.
	pub fn forward_kinematics(&self, anim: &[Isometry3], world: &mut Vec<Isometry3>, skin: &mut Vec<Isometry3>) {
		world.clear();
		skin.clear();
		for id in 0..anim.len() {
			let local = Isometry3::from_parts(self.mmd_local[id], Rot3::identity()) * anim[id];
			let transform = match self.mmd_parents[id] {
				Some(parent) if parent < world.len() => world[parent] * local,
				_ => Isometry3::from_parts(self.mmd_rest[id], Rot3::identity()) * anim[id],
			};
			world.push(transform);
			skin.push(transform * Isometry3::from_parts(self.mmd_rest[id].inverse(), Rot3::identity()));
		}
	}
}

#[derive(Debug, Error)]
pub enum RetargetError {
	#[error("Unknown humanoid bone `{0}`")] UnknownBone(String),
	#[error("No such MMD bone {0}")] NoSuchMMDBone(usize),
	#[error("Humanoid bone `{0}` must be mapped")] MissingBone(String),
}
