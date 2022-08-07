use std::path::{PathBuf, Path};
use std::marker::PhantomData;
use std::fmt::{Display, Formatter};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::ErrorKind;
use serde::de::DeserializeOwned;
use err_derive::Error;

use crate::renderer::Renderer;
use super::{AssetKey, AssetsManager, AssetError};

#[derive(Clone, Debug)]
pub struct TomlAsset<T> {
	path: PathBuf,
	phantom: PhantomData<T>,
}

impl<T> TomlAsset<T> {
	pub fn at(path: impl AsRef<Path>) -> Self {
		TomlAsset {
			path: path.as_ref().to_path_buf(),
			phantom: PhantomData,
		}
	}
}

impl<T> AssetKey for TomlAsset<T>
	where T: DeserializeOwned + Clone + 'static {
	type Asset = T;
	type Error = TomlLoadError;
	
	fn load(&self, _assets_manager: &mut AssetsManager, _renderer: &mut Renderer) -> Result<Self::Asset, Self::Error> {
		let file = fs::read_to_string(AssetsManager::find_asset_path(&self.path)?)?;
		let data = toml::from_str(&file)?;
		
		Ok(data)
	}
}

impl<T> Display for TomlAsset<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "TOML file {}", self.path.to_string_lossy())
	}
}

impl<T> Hash for TomlAsset<T> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.path.hash(state)
	}
}

#[derive(Debug, Error)]
pub enum TomlLoadError {
	#[error(display = "{}", _0)] AssetError(#[error(source)] AssetError),
	#[error(display = "{}", _0)] DeserializationError(#[error(source)] toml::de::Error),
	#[error(display = "{}", _0)] IoError(#[error(source)] std::io::Error),
}

impl TomlLoadError {
	pub fn kind(&self) -> ErrorKind {
		match self {
			TomlLoadError::AssetError(err) => err.kind(),
			TomlLoadError::IoError(err) => err.kind(),
			_ => ErrorKind::Other,
		}
	}
}
