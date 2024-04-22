use std::fmt::{Display, Formatter};
use std::path::{PathBuf, Path};
use anyhow::Result;
use obj::Obj;

use crate::renderer::Renderer;
use crate::renderer::assets_manager::{AssetKey, AssetsManager, TextureAsset};
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
	
	fn load(&self, assets_manager: &mut AssetsManager, renderer: &mut Renderer) -> Result<Self::Asset> {
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
