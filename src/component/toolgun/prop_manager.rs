use std::fs;
use std::collections::HashMap;
use err_derive::Error;
use serde_derive::Deserialize;

use crate::component::model::SimpleModel;
use crate::component::model::simple::SimpleModelLoadError;
use crate::utils::{AssetError, find_asset_path};
use crate::renderer::Renderer;

#[derive(Deserialize)]
pub struct PropConfig {
	model: String,
	texture: String,
}

pub struct PropManager {
	pub props: Vec<SimpleModel<u32>>,
}

impl PropManager {
	pub fn new(renderer: &mut Renderer) -> Result<Self, PropManagerError> {
		let mut props = Vec::new();
		
		let config: HashMap<String, PropConfig> = toml::from_str(&fs::read_to_string(find_asset_path("props.toml")?)?)?;
		
		for (_, pconf) in config {
			props.push(SimpleModel::from_obj(&pconf.model, &pconf.texture, renderer)?);
		}
		
		Ok(PropManager {
			props,
		})
	}
}


#[derive(Debug, Error)]
pub enum PropManagerError {
	#[error(display = "{}", _0)] AssetError(#[error(source)] AssetError),
	#[error(display = "{}", _0)] SimpleModelLoadError(#[error(source)] SimpleModelLoadError),
	#[error(display = "{}", _0)] IoError(#[error(source)] std::io::Error),
	#[error(display = "{}", _0)] DeserializationError(#[error(source)] toml::de::Error),
}

