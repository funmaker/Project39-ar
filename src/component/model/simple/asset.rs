use std::path::{PathBuf, Path};
use std::fmt::{Display, Formatter};
use std::io::ErrorKind;
use err_derive::Error;
use obj::Obj;

use crate::renderer::Renderer;
use crate::renderer::assets_manager::{AssetError, AssetKey, AssetsManager, TextureAsset, TextureLoadError};
use crate::component::model::ModelError;
use super::{SimpleModel, Vertex};

#[derive(Clone, Hash, Debug)]
pub struct ObjAsset {
	model: PathBuf,
	texture: TextureAsset,
}

impl ObjAsset {
	pub fn at(model_path: impl AsRef<Path>, texture: impl Into<TextureAsset>) -> Self {
		ObjAsset {
			model: model_path.as_ref().to_path_buf(),
			texture: texture.into(),
		}
	}
}

impl AssetKey for ObjAsset {
	type Asset = SimpleModel;
	type Error = ObjLoadError;
	
	fn load(&self, assets_manager: &mut AssetsManager, renderer: &mut Renderer) -> Result<Self::Asset, Self::Error> {
		let texture = assets_manager.load(self.texture.clone(), renderer)?;
		let model: Obj<obj::TexturedVertex, u32> = obj::load_obj(AssetsManager::find_asset(&self.model)?)?;
		
		Ok(SimpleModel::new(
			&model.vertices.iter().map(Into::into).collect::<Vec<_>>(),
			&model.indices,
			texture,
			renderer,
		)?)
	}
}

impl Display for ObjAsset {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "OBJ model {} ({})", self.model.to_string_lossy(), self.texture)
	}
}

impl From<&obj::TexturedVertex> for Vertex {
	fn from(vertex: &obj::TexturedVertex) -> Self {
		Vertex::new(
			vertex.position,
			vertex.normal,
			[vertex.texture[0], 1.0 - vertex.texture[1]]
		)
	}
}

#[derive(Debug, Error)]
pub enum ObjLoadError {
	#[error(display = "{}", _0)] AssetError(#[error(source)] AssetError),
	#[error(display = "{}", _0)] TextureLoadError(#[error(source)] TextureLoadError),
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] ObjError(#[error(source)] obj::ObjError),
}

impl ObjLoadError {
	pub fn kind(&self) -> ErrorKind {
		match self {
			ObjLoadError::AssetError(err) => err.kind(),
			ObjLoadError::TextureLoadError(err) => err.kind(),
			_ => ErrorKind::Other,
		}
	}
}
