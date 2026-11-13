
//! Skeleton independent humanoid bone set the imported animations are stored in.
//!
//! Matches `HUMANOID` in `tools/openviva/import.py`. In the humanoid space every bone has an identity rest
//! rotation and the character faces +Z with +X to its left and +Y up.

use std::sync::LazyLock;


pub const HIPS: usize = 0;
pub const SPINE: usize = 1;
pub const CHEST: usize = 2;
pub const NECK: usize = 3;
pub const HEAD: usize = 4;

/// Offset of the right side bones from their left side counterparts.
const SIDE_OFFSET: usize = 24;

pub const FINGERS: [&str; 5] = ["Thumb", "Index", "Middle", "Ring", "Little"];
const FINGER_PHALANGES: [&str; 3] = ["Proximal", "Intermediate", "Distal"];

/// Bones of one side of the body, relative to the first bone of the side.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Side {
	Left,
	Right,
}

impl Side {
	fn offset(self) -> usize {
		match self {
			Side::Left => 5,
			Side::Right => 5 + SIDE_OFFSET,
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Side::Left => "Left",
			Side::Right => "Right",
		}
	}

	pub fn other(self) -> Side {
		match self {
			Side::Left => Side::Right,
			Side::Right => Side::Left,
		}
	}

	pub fn eye(self) -> usize { self.offset() }
	pub fn shoulder(self) -> usize { self.offset() + 1 }
	pub fn upper_arm(self) -> usize { self.offset() + 2 }
	pub fn lower_arm(self) -> usize { self.offset() + 3 }
	pub fn hand(self) -> usize { self.offset() + 4 }
	/// Finger bone, `finger` 0..5 (thumb to little), `phalange` 0..3.
	pub fn finger(self, finger: usize, phalange: usize) -> usize { self.offset() + 5 + finger * 3 + phalange }
	pub fn upper_leg(self) -> usize { self.offset() + 20 }
	pub fn lower_leg(self) -> usize { self.offset() + 21 }
	pub fn foot(self) -> usize { self.offset() + 22 }
	pub fn toes(self) -> usize { self.offset() + 23 }
}

/// A bone of the humanoid skeleton.
#[derive(Clone, Debug)]
pub struct HumanBone {
	pub name: String,
	pub parent: Option<usize>,
	/// Bone the direction of this bone points to, used to align the rest poses when retargeting.
	pub tip: Option<usize>,
	/// The same bone on the other side of the body.
	pub mirror: usize,
}

/// All humanoid bones, parents before children.
pub static BONES: LazyLock<Vec<HumanBone>> = LazyLock::new(|| {
	let mut bones = vec![];
	let mut push = |name: String, parent: Option<usize>, tip: Option<usize>| {
		let id = bones.len();
		bones.push(HumanBone { name, parent, tip, mirror: id });
	};

	push("Hips".into(), None, Some(SPINE));
	push("Spine".into(), Some(HIPS), Some(CHEST));
	push("Chest".into(), Some(SPINE), Some(NECK));
	push("Neck".into(), Some(CHEST), Some(HEAD));
	push("Head".into(), Some(NECK), None);

	for side in [Side::Left, Side::Right] {
		let s = side.name();
		push(format!("{s}Eye"), Some(HEAD), None);
		push(format!("{s}Shoulder"), Some(CHEST), Some(side.upper_arm()));
		push(format!("{s}UpperArm"), Some(side.shoulder()), Some(side.lower_arm()));
		push(format!("{s}LowerArm"), Some(side.upper_arm()), Some(side.hand()));
		push(format!("{s}Hand"), Some(side.lower_arm()), Some(side.finger(2, 0)));
		for (f, finger) in FINGERS.iter().enumerate() {
			for (p, phalange) in FINGER_PHALANGES.iter().enumerate() {
				let parent = if p == 0 { side.hand() } else { side.finger(f, p - 1) };
				let tip = (p < 2).then(|| side.finger(f, p + 1));
				push(format!("{s}{finger}{phalange}"), Some(parent), tip);
			}
		}
		push(format!("{s}UpperLeg"), Some(HIPS), Some(side.lower_leg()));
		push(format!("{s}LowerLeg"), Some(side.upper_leg()), Some(side.foot()));
		push(format!("{s}Foot"), Some(side.lower_leg()), Some(side.toes()));
		push(format!("{s}Toes"), Some(side.foot()), None);
	}

	for id in 0..bones.len() {
		if id >= Side::Right.offset() {
			bones[id].mirror = id - SIDE_OFFSET;
		} else if id >= Side::Left.offset() {
			bones[id].mirror = id + SIDE_OFFSET;
		}
	}

	bones
});

pub fn count() -> usize {
	BONES.len()
}

/// Finds a bone by name.
pub fn find(name: &str) -> Option<usize> {
	BONES.iter().position(|bone| bone.name == name)
}

/// Returns `true` if the bone is controlled by the upper body ("torso") animation layer.
pub fn is_upper_body(id: usize) -> bool {
	if id == HIPS {
		false
	} else if id < Side::Left.offset() {
		true
	} else {
		(id - Side::Left.offset()) % SIDE_OFFSET < 20
	}
}

/// Returns `true` for the eyes, which are controlled by the face animation layer.
pub fn is_eye(id: usize) -> bool {
	id == Side::Left.eye() || id == Side::Right.eye()
}

/// Blend shapes of the source model, animated by clips and mapped to model morphs.
pub const MORPHS: [&str; 27] = [
	"centerLip", "cheek_l", "cheek_r", "frown_l", "frown_r", "happyClosedEyelids", "jawDown", "lowerEyelid_l",
	"lowerEyelid_r", "lowerLip_l", "lowerLip_r", "madEye_l", "madEye_r", "madEyebrow_l", "madEyebrow_r", "platysma",
	"puckerLips", "pupilShrink", "sadEyebrow_l", "sadEyebrow_r", "showBotTeeth", "smile_l", "smile_r", "upperEyelid_l",
	"upperEyelid_r", "upperLip_l", "upperLip_r",
];

pub fn find_morph(name: &str) -> Option<usize> {
	MORPHS.iter().position(|morph| *morph == name)
}

/// Morph on the other side of the face.
pub fn mirror_morph(id: usize) -> usize {
	let name = MORPHS[id];
	let mirrored = if let Some(base) = name.strip_suffix("_l") {
		format!("{base}_r")
	} else if let Some(base) = name.strip_suffix("_r") {
		format!("{base}_l")
	} else {
		return id;
	};
	find_morph(&mirrored).unwrap_or(id)
}
