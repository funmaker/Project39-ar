use std::fmt::{Display, Formatter};
use std::io::ErrorKind;
use std::path::{PathBuf, Path};
use std::sync::Arc;
use err_derive::Error;
use image::{ImageFormat, DynamicImage, ImageDecoder, AnimationDecoder, RgbaImage, imageops};
use image::codecs::gif::GifDecoder;
use vulkano::{command_buffer, sampler, sync};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract};
use vulkano::format::Format;
use vulkano::image::{ImmutableImage, ImageDimensions, MipmapsCount};
use vulkano::image::view::ImageView;
use vulkano::sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode};

use crate::utils::{FenceCheck, ImageEx};
use super::super::Renderer;
use super::{AssetError, AssetKey, AssetsManager};


#[derive(Clone, Hash, Debug)]
pub struct TextureAsset {
	path: PathBuf,
	filter: Filter,
	mipmaps: bool,
	srgb: bool,
}

#[derive(Clone)]
pub struct TextureBundle {
	pub image: Arc<ImageView<ImmutableImage>>,
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
			srgb: true,
		}
	}
	
	pub fn nearest(self) -> Self {
		TextureAsset {
			filter: Filter::Nearest,
			..self
		}
	}
	
	pub fn no_mipmaps(self) -> Self {
		TextureAsset {
			mipmaps: false,
			..self
		}
	}
	
	pub fn no_srgb(self) -> Self {
		TextureAsset {
			srgb: false,
			..self
		}
	}
}

impl AssetKey for TextureAsset {
	type Asset = TextureBundle;
	type Error = TextureLoadError;
	
	fn load(&self, _assets_manager: &mut AssetsManager, renderer: &mut Renderer) -> Result<Self::Asset, Self::Error> {
		let file_format = ImageFormat::from_path(&self.path)?;
		let mip_levels = if self.mipmaps { MipmapsCount::Log2 } else { MipmapsCount::One };
		let format = if self.srgb { Format::R8G8B8A8_SRGB } else { Format::R8G8B8A8_UNORM };
		
		let mut upload_buffer = AutoCommandBufferBuilder::primary(&*renderer.command_buffer_allocator,
		                                                          renderer.load_queue.queue_family_index(),
		                                                          CommandBufferUsage::OneTimeSubmit)?;
		
		let image = if file_format == ImageFormat::Gif {
			let decoder = GifDecoder::new(AssetsManager::find_asset(&self.path)?)?;
			let (width, height) = decoder.dimensions();
			let frames = decoder.into_frames().collect_frames()?;
			let array_layers = frames.len() as u32;
			
			let pixels = frames.into_iter()
			                   .flat_map(|frame| {
				                   let mut canvas = RgbaImage::new(width, height);
				                   imageops::replace(&mut canvas, frame.buffer(), frame.left() as i64, frame.top() as i64);
				                   if self.srgb {
					                   DynamicImage::from(canvas).into_pre_mul_iter()
				                   } else {
					                   DynamicImage::from(canvas).into_lin_pre_mul_iter()
				                   }
			                   })
			                   .collect::<Vec<_>>();
			
			ImmutableImage::from_iter(&renderer.memory_allocator,
			                          pixels.into_iter(),
			                          ImageDimensions::Dim2d{ width, height, array_layers },
			                          mip_levels,
			                          format,
			                          &mut upload_buffer)?
		} else {
			let image = image::load(AssetsManager::find_asset(&self.path)?, file_format)?;
			let width = image.width();
			let height = image.height();
			
			ImmutableImage::from_iter(&renderer.memory_allocator,
			                          image.into_pre_mul_iter(),
			                          ImageDimensions::Dim2d{ width, height, array_layers: 1 },
			                          mip_levels,
			                          format,
			                          &mut upload_buffer)?
		};
		
		
		let upload_future = upload_buffer.build()?
		                                 .execute(renderer.load_queue.clone())?;
		
		let sampler = Sampler::new(renderer.device.clone(), SamplerCreateInfo {
			mag_filter: self.filter,
			min_filter: self.filter,
			mipmap_mode: if self.mipmaps { SamplerMipmapMode::Linear } else { SamplerMipmapMode::Nearest },
			address_mode: [SamplerAddressMode::Repeat; 3],
			lod: if self.mipmaps { 0.0..=1000.0 } else { 0.0..=1.0 },
			..SamplerCreateInfo::default()
		})?;
		
		let view = ImageView::new_default(image.clone())?;
		let fence = FenceCheck::new(upload_future)?;
		
		Ok(TextureBundle {
			image: view,
			sampler,
			fence,
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
		
		// TODO: Batching of all upload_buffers?
		let mut upload_buffer = AutoCommandBufferBuilder::primary(&*renderer.command_buffer_allocator,
		                                                          renderer.load_queue.queue_family_index(),
		                                                          CommandBufferUsage::OneTimeSubmit)?;
		
		let image = ImmutableImage::from_iter(&renderer.memory_allocator,
		                                      source.into_pre_mul_iter(),
		                                      ImageDimensions::Dim2d{ width, height, array_layers: 1 },
		                                      MipmapsCount::Log2,
		                                      Format::R8G8B8A8_SRGB,
		                                      &mut upload_buffer)?;
		
		let upload_future = upload_buffer.build()?
		                                 .execute(renderer.load_queue.clone())?;
		
		let view = ImageView::new_default(image.clone())?;
		let sampler = Sampler::new(renderer.device.clone(), SamplerCreateInfo::simple_repeat_linear())?;
		let fence = FenceCheck::new(upload_future)?;
		
		Ok(TextureBundle {
			image: view,
			sampler,
			fence,
		})
	}
}

#[derive(Debug, Error)]
pub enum TextureLoadError {
	#[error(display = "{}", _0)] AssetError(#[error(source)] AssetError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] ImmutableImageCreationError(#[error(source)] vulkano::image::immutable::ImmutableImageCreationError),
	#[error(display = "{}", _0)] CommandBufferBeginError(#[error(source)] command_buffer::CommandBufferBeginError),
	#[error(display = "{}", _0)] BuildError(#[error(source)] command_buffer::BuildError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] command_buffer::CommandBufferExecError),
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] SamplerCreationError(#[error(source)] sampler::SamplerCreationError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
}

impl TextureLoadError {
	pub fn kind(&self) -> ErrorKind {
		match self {
			TextureLoadError::AssetError(err) => err.kind(),
			_ => ErrorKind::Other,
		}
	}
}
