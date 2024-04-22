use std::fs;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::path::{PathBuf, Path};
use anyhow::Result;
use serde::de::DeserializeOwned;

use super::super::Renderer;
use super::{AssetKey, AssetsManager};


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
	
	fn load(&self, _assets_manager: &mut AssetsManager, _renderer: &mut Renderer) -> Result<Self::Asset> {
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
