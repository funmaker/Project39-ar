use std::fmt::{Display, Formatter};
use std::path::{PathBuf, Path};
use std::time::Duration;
use anyhow::Result;
use rodio::Source;
use rodio::buffer::SamplesBuffer;
use thiserror::Error;

use super::super::Renderer;
use super::{AssetKey, AssetsManager};


/// Decoded sound, ready to be played by `Audio`. Cheap to clone.
#[derive(Clone, Debug)]
pub struct Sound {
	pub(crate) buffer: SamplesBuffer,
	pub duration: Duration,
}

/// Key of a sound file (ogg vorbis or wav), decoded into memory on load.
#[derive(Clone, Debug, Hash)]
pub struct SoundAsset {
	path: PathBuf,
}

impl SoundAsset {
	pub fn at(path: impl AsRef<Path>) -> Self {
		SoundAsset {
			path: path.as_ref().to_path_buf(),
		}
	}
}

impl AssetKey for SoundAsset {
	type Asset = Sound;

	fn load(&self, _assets_manager: &mut AssetsManager, _renderer: &mut Renderer) -> Result<Self::Asset> {
		let file = std::fs::File::open(AssetsManager::find_asset_path(&self.path)?)?;
		let decoder = rodio::Decoder::try_from(file).map_err(|err| SoundDecodeError(self.path.to_string_lossy().to_string(), err.to_string()))?;
		let channels = decoder.channels();
		let sample_rate = decoder.sample_rate();
		let samples: Vec<_> = decoder.collect();
		let buffer = SamplesBuffer::new(channels, sample_rate, samples);
		let duration = buffer.total_duration().unwrap_or_default();

		Ok(Sound { buffer, duration })
	}
}

impl Display for SoundAsset {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "Sound {}", self.path.to_string_lossy())
	}
}

/// The sound file could not be decoded.
#[derive(Debug, Error)]
#[error("Unable to decode sound `{0}`: {1}")]
pub struct SoundDecodeError(String, String);
