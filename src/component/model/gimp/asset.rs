use std::path::{PathBuf, Path};
use std::fmt::{Display, Formatter};
use std::io::ErrorKind;
use err_derive::Error;
use obj::Obj;

use crate::renderer::Renderer;
use crate::renderer::assets_manager::texture::{TextureAsset, TextureLoadError};
use crate::renderer::assets_manager::{AssetError, AssetKey, AssetsManager};
use crate::component::model::ModelError;
use super::{GimpModel, Vertex};

#[derive(Clone, Hash, Debug)]
pub struct GimpAsset {
	model: PathBuf,
	texture: TextureAsset,
	norm_texture: TextureAsset,
}

impl GimpAsset {
	pub fn at(model_path: impl AsRef<Path>, texture: impl Into<TextureAsset>, norm_texture: impl Into<TextureAsset>) -> Self {
		GimpAsset {
			model: model_path.as_ref().to_path_buf(),
			texture: texture.into(),
			norm_texture: norm_texture.into(),
		}
	}
}

impl AssetKey for GimpAsset {
	type Asset = GimpModel;
	type Error = GimpLoadError;
	
	fn load(&self, assets_manager: &mut AssetsManager, renderer: &mut Renderer) -> Result<Self::Asset, Self::Error> {
		let texture = assets_manager.load(self.texture.clone(), renderer)?;
		let norm_texture = assets_manager.load(self.norm_texture.clone(), renderer)?;
		let model: Obj<obj::TexturedVertex, u32> = obj::load_obj(AssetsManager::find_asset(&self.model)?)?;
		
		Ok(GimpModel::new(
			&model.vertices.iter().map(Into::into).collect::<Vec<_>>(),
			&model.indices,
			texture,
			norm_texture,
			renderer,
		)?)
	}
}

impl Display for GimpAsset {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "GIMP model {} ({})", self.model.to_string_lossy(), self.texture)
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
pub enum GimpLoadError {
	#[error(display = "{}", _0)] AssetError(#[error(source)] AssetError),
	#[error(display = "{}", _0)] TextureLoadError(#[error(source)] TextureLoadError),
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] ObjError(#[error(source)] obj::ObjError),
}

impl GimpLoadError {
	pub fn kind(&self) -> ErrorKind {
		match self {
			GimpLoadError::AssetError(err) => err.kind(),
			GimpLoadError::TextureLoadError(err) => err.kind(),
			_ => ErrorKind::Other,
		}
	}
}