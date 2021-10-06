use std::sync::Arc;
use std::path::{PathBuf, Path};
use std::fmt::{Display, Formatter};
use err_derive::Error;
use image::{ImageFormat, GenericImageView};
use vulkano::image::{ImmutableImage, ImageDimensions, MipmapsCount};
use vulkano::format::Format;

use crate::renderer::Renderer;
use crate::utils::{FenceCheck, ImageEx};
use super::{AssetError, AssetKey, AssetsManager};

#[derive(Clone, Hash, Debug)]
pub struct TextureAsset {
	path: PathBuf,
}

impl TextureAsset {
	pub fn at(path: impl AsRef<Path>) -> Self {
		TextureAsset {
			path: path.as_ref().to_path_buf()
		}
	}
}

impl AssetKey for TextureAsset {
	type Asset = (Arc<ImmutableImage>, FenceCheck);
	type Error = TextureLoadError;
	
	fn load(&self, _assets_manager: &mut AssetsManager, renderer: &mut Renderer) -> Result<Self::Asset, Self::Error> {
		let source = image::load(AssetsManager::find_asset(&self.path)?, ImageFormat::from_path(&self.path)?)?;
		let width = source.width();
		let height = source.height();
		
		let (texture, texture_promise) = ImmutableImage::from_iter(source.into_pre_mul_iter(),
		                                                           ImageDimensions::Dim2d{ width, height, array_layers: 1 },
		                                                           MipmapsCount::Log2,
		                                                           Format::R8G8B8A8_UNORM,
		                                                           renderer.load_queue.clone())?;
		
		Ok((texture, FenceCheck::new(texture_promise)?))
	}
}

impl Display for TextureAsset {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "texture {}", self.path.to_string_lossy())
	}
}

#[derive(Debug, Error)]
pub enum TextureLoadError {
	#[error(display = "{}", _0)] AssetError(#[error(source)] AssetError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] vulkano::image::ImageCreationError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] vulkano::sync::FlushError),
}
