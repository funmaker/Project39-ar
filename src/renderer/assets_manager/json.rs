use std::fs;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::path::{PathBuf, Path};
use std::sync::Arc;
use anyhow::Result;
use serde::de::DeserializeOwned;

use super::super::Renderer;
use super::{AssetKey, AssetsManager};


/// Key of a JSON file deserialized into `T`. The loaded value is shared, since data files can be large.
#[derive(Clone, Debug)]
pub struct JsonAsset<T> {
	path: PathBuf,
	phantom: PhantomData<T>,
}

impl<T> JsonAsset<T> {
	pub fn at(path: impl AsRef<Path>) -> Self {
		JsonAsset {
			path: path.as_ref().to_path_buf(),
			phantom: PhantomData,
		}
	}
}

impl<T> AssetKey for JsonAsset<T>
	where T: DeserializeOwned + 'static {
	type Asset = Arc<T>;

	fn load(&self, _assets_manager: &mut AssetsManager, _renderer: &mut Renderer) -> Result<Self::Asset> {
		let file = fs::read_to_string(AssetsManager::find_asset_path(&self.path)?)?;
		let data = serde_json::from_str(&file)?;

		Ok(Arc::new(data))
	}
}

impl<T> Display for JsonAsset<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "JSON file {}", self.path.to_string_lossy())
	}
}

impl<T> Hash for JsonAsset<T> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.path.hash(state)
	}
}
