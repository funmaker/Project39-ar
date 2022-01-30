use std::sync::Arc;
use std::path::{PathBuf, Path};
use std::fmt::{Display, Formatter};
use err_derive::Error;
use image::{ImageFormat, GenericImageView, DynamicImage};
use vulkano::image::{ImmutableImage, ImageDimensions, MipmapsCount};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode};

use crate::renderer::Renderer;
use crate::utils::{FenceCheck, ImageEx};
use super::{AssetError, AssetKey, AssetsManager};

#[derive(Clone, Hash, Debug)]
pub struct TextureAsset {
	path: PathBuf,
	filter: Filter,
	mipmaps: bool,
}

#[derive(Clone)]
pub struct TextureBundle {
	pub image: Arc<ImmutableImage>,
	pub view: Arc<ImageView<ImmutableImage>>,
	pub sampler: Arc<Sampler>,
	pub fence: FenceCheck,
}

#[allow(unused)]
impl TextureAsset {
	pub fn at(path: impl AsRef<Path>) -> Self {
		TextureAsset {
			path: path.as_ref().to_path_buf(),
			filter: Filter::Linear,
			mipmaps: true,
		}
	}
	
	pub fn no_mipmaps(self) -> Self {
		TextureAsset {
			mipmaps: false,
			..self
		}
	}
	
	pub fn nearest(self) -> Self {
		TextureAsset {
			filter: Filter::Nearest,
			..self
		}
	}
}

impl AssetKey for TextureAsset {
	type Asset = TextureBundle;
	type Error = TextureLoadError;
	
	fn load(&self, _assets_manager: &mut AssetsManager, renderer: &mut Renderer) -> Result<Self::Asset, Self::Error> {
		let source = image::load(AssetsManager::find_asset(&self.path)?, ImageFormat::from_path(&self.path)?)?;
		let width = source.width();
		let height = source.height();
		
		let (image, image_promise) = ImmutableImage::from_iter(source.into_pre_mul_iter(),
		                                                       ImageDimensions::Dim2d{ width, height, array_layers: 1 },
		                                                       if self.mipmaps { MipmapsCount::Log2 } else { MipmapsCount::One },
		                                                       Format::R8G8B8A8_UNORM,
		                                                       renderer.load_queue.clone())?;
		
		let sampler = Sampler::new(renderer.device.clone(),
		                           self.filter,
		                           self.filter,
		                           if self.mipmaps { MipmapMode::Linear } else { MipmapMode::Linear },
		                           SamplerAddressMode::Repeat,
		                           SamplerAddressMode::Repeat,
		                           SamplerAddressMode::Repeat,
		                           0.0,
		                           1.0,
		                           0.0,
		                           if self.mipmaps { 1000.0 } else { 1.0 })?;
		
		let view = ImageView::new(image.clone())?;
		let fence = FenceCheck::new(image_promise)?;
		
		Ok(TextureBundle {
			image,
			view,
			sampler,
			fence
		})
	}
}

impl Display for TextureAsset {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "texture {}", self.path.to_string_lossy())
	}
}

impl<T> From<T> for TextureAsset
where T: AsRef<Path> {
	fn from(path: T) -> Self {
		TextureAsset::at(path)
	}
}

impl TextureBundle {
	pub fn from_raw_simple(source: DynamicImage, renderer: &Renderer) -> Result<TextureBundle, TextureLoadError> {
		let width = source.width();
		let height = source.height();
		
		let (image, image_promise) = ImmutableImage::from_iter(source.into_pre_mul_iter(),
		                                                       ImageDimensions::Dim2d{ width, height, array_layers: 1 },
		                                                       MipmapsCount::Log2,
		                                                       Format::R8G8B8A8_UNORM,
		                                                       renderer.load_queue.clone())?;
		
		let view = ImageView::new(image.clone())?;
		let sampler = Sampler::simple_repeat_linear(renderer.device.clone());
		let fence = FenceCheck::new(image_promise)?;
		
		Ok(TextureBundle {
			image,
			view,
			sampler,
			fence,
		})
	}
}

#[derive(Debug, Error)]
pub enum TextureLoadError {
	#[error(display = "{}", _0)] AssetError(#[error(source)] AssetError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] vulkano::image::ImageCreationError),
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] SamplerCreationError(#[error(source)] vulkano::sampler::SamplerCreationError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] vulkano::sync::FlushError),
}
