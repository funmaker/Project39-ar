use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::Result;
use nalgebra::Quaternion;
use thiserror::Error;

use crate::math::{Rot3, Vec3};
use crate::renderer::Renderer;
use crate::renderer::assets_manager::{AssetKey, AssetsManager};
use super::human;
use super::pose::{nlerp, Pose};


/// Animation clip baked in the humanoid space by `tools/openviva/import.py` (`.p39a` files).
#[derive(Debug)]
pub struct Clip {
	pub frame_rate: f32,
	pub frames: usize,
	pub looping: bool,
	/// `frames * human::count()` local bone rotations.
	rotations: Vec<Rot3>,
	translations: Vec<Vec3>,
	/// `frames * human::MORPHS.len()` blend shape weights.
	morphs: Vec<f32>,
}

impl Clip {
	/// Length in seconds.
	pub fn length(&self) -> f32 {
		(self.frames.max(2) - 1) as f32 / self.frame_rate
	}

	/// Samples the clip at `time` seconds into `out`. The time is wrapped if the clip loops and clamped otherwise.
	pub fn sample(&self, time: f32, out: &mut Pose) {
		let length = self.length();
		let time = if self.looping && length > 0.0 { time.rem_euclid(length) } else { time.clamp(0.0, length) };
		let frame = time * self.frame_rate;
		let a = (frame.floor() as usize).min(self.frames - 1);
		let b = (a + 1).min(self.frames - 1);
		let t = frame - a as f32;
		let bones = human::count();
		let morphs = human::MORPHS.len();

		for id in 0..bones {
			out.rotations[id] = nlerp(self.rotations[a * bones + id], self.rotations[b * bones + id], t);
		}
		out.root = self.translations[a].lerp(&self.translations[b], t);
		for id in 0..morphs {
			out.morphs[id] = self.morphs[a * morphs + id] * (1.0 - t) + self.morphs[b * morphs + id] * t;
		}
	}

	fn parse(data: &[u8]) -> Result<Clip, ClipError> {
		let mut reader = Reader(data);
		if reader.bytes(4)? != b"P39A" { return Err(ClipError::BadMagic); }
		if reader.u32()? != 1 { return Err(ClipError::UnsupportedVersion); }
		let frame_rate = reader.f32()?;
		let frames = reader.u32()? as usize;
		let looping = reader.u8()? != 0;
		if frames == 0 { return Err(ClipError::Empty); }

		let bone_count = reader.u16()? as usize;
		let bone_map = (0..bone_count).map(|_| Ok(human::find(&reader.string()?)))
		                              .collect::<Result<Vec<_>, ClipError>>()?;
		let morph_count = reader.u16()? as usize;
		let morph_map = (0..morph_count).map(|_| Ok(human::find_morph(&reader.string()?)))
		                                .collect::<Result<Vec<_>, ClipError>>()?;

		let bones = human::count();
		let morphs = human::MORPHS.len();
		let mut rotations = vec![Rot3::identity(); frames * bones];
		let mut translations = Vec::with_capacity(frames);
		let mut morph_values = vec![0.0; frames * morphs];

		for frame in 0..frames {
			for id in bone_map.iter() {
				let q = [reader.i16()?, reader.i16()?, reader.i16()?, reader.i16()?].map(|v| v as f32 / 32767.0);
				if let Some(id) = id {
					rotations[frame * bones + id] = Rot3::new_normalize(Quaternion::new(q[3], q[0], q[1], q[2]));
				}
			}
			translations.push(vector!(reader.f32()?, reader.f32()?, reader.f32()?));
			for id in morph_map.iter() {
				let value = reader.u8()? as f32 / 255.0;
				if let Some(id) = id {
					morph_values[frame * morphs + id] = value;
				}
			}
		}

		Ok(Clip {
			frame_rate,
			frames,
			looping,
			rotations,
			translations,
			morphs: morph_values,
		})
	}
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
	fn bytes(&mut self, len: usize) -> Result<&'a [u8], ClipError> {
		if self.0.len() < len { return Err(ClipError::UnexpectedEof); }
		let (head, tail) = self.0.split_at(len);
		self.0 = tail;
		Ok(head)
	}

	fn u8(&mut self) -> Result<u8, ClipError> { Ok(self.bytes(1)?[0]) }
	fn u16(&mut self) -> Result<u16, ClipError> { Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap())) }
	fn i16(&mut self) -> Result<i16, ClipError> { Ok(i16::from_le_bytes(self.bytes(2)?.try_into().unwrap())) }
	fn u32(&mut self) -> Result<u32, ClipError> { Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap())) }
	fn f32(&mut self) -> Result<f32, ClipError> { Ok(f32::from_le_bytes(self.bytes(4)?.try_into().unwrap())) }

	fn string(&mut self) -> Result<String, ClipError> {
		let len = self.u8()? as usize;
		Ok(String::from_utf8_lossy(self.bytes(len)?).into_owned())
	}
}

/// Key of a `.p39a` animation clip.
#[derive(Clone, Debug, Hash)]
pub struct ClipAsset {
	path: PathBuf,
}

impl ClipAsset {
	pub fn at(path: impl AsRef<Path>) -> Self {
		ClipAsset { path: path.as_ref().to_path_buf() }
	}
}

impl AssetKey for ClipAsset {
	type Asset = Arc<Clip>;

	fn load(&self, _assets_manager: &mut AssetsManager, _renderer: &mut Renderer) -> Result<Self::Asset> {
		let mut data = vec![];
		AssetsManager::find_asset(&self.path)?.read_to_end(&mut data)?;
		Ok(Arc::new(Clip::parse(&data).map_err(|err| ClipLoadError(self.path.to_string_lossy().into_owned(), err))?))
	}
}

impl Display for ClipAsset {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "Animation clip {}", self.path.to_string_lossy())
	}
}

#[derive(Debug, Error)]
pub enum ClipError {
	#[error("Not an animation clip")] BadMagic,
	#[error("Unsupported clip version")] UnsupportedVersion,
	#[error("Clip has no frames")] Empty,
	#[error("Unexpected end of file")] UnexpectedEof,
}

#[derive(Debug, Error)]
#[error("Unable to load animation clip `{0}`: {1}")]
pub struct ClipLoadError(String, ClipError);
