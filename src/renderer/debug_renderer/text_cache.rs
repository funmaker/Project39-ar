use std::collections::HashMap;
use std::sync::Arc;
use err_derive::Error;
use vulkano::descriptor_set::{self, PersistentDescriptorSet, DescriptorSet};
use vulkano::sampler::{Sampler, Filter, MipmapMode, SamplerAddressMode, BorderColor};
use vulkano::image::{ImmutableImage, MipmapsCount, ImageDimensions, view::ImageView};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::device::Queue;
use vulkano::sampler;
use unifont::Glyph;

use crate::renderer::pipelines::debug::DebugTexturedPipeline;
use crate::renderer::pipelines::Pipelines;
use crate::renderer::pipelines::PipelineError;

pub struct TextCache {
	pipeline: Arc<GraphicsPipeline>,
	queue: Arc<Queue>,
	entries: HashMap<String, TextEntry>,
}

// Unifont's U+FFFD � REPLACEMENT CHARACTER
const REPLACEMENT_GLYPH: Glyph = Glyph::HalfWidth([
	0x00, 0x00, 0x00, 0x7E, 0x66, 0x5A, 0x5A, 0x7A, 0x76, 0x76, 0x7E, 0x76, 0x76, 0x7E, 0x00, 0x00
]);

impl TextCache {
	pub fn new(queue: &Arc<Queue>, pipelines: &mut Pipelines) -> Result<Self, TextCacheError> {
		let pipeline = pipelines.get::<DebugTexturedPipeline>()?;
		
		Ok(TextCache {
			pipeline,
			queue: queue.clone(),
			entries: HashMap::new(),
		})
	}
	
	pub fn get(&mut self, text: &'_ str) -> Result<TextEntry, TextCacheGetError> {
		if let Some(entry) = self.entries.get(text) {
			Ok(entry.clone())
		} else {
			let glyphs = text.chars()
			                 .map(|c| unifont::get_glyph(c).unwrap_or(&REPLACEMENT_GLYPH))
			                 .collect::<Vec<_>>();
			
			let width = glyphs.iter().fold(0, |acc, g| acc + g.get_width() as u32);
			let height = 16;
			
			let data = Rasterizer::new(glyphs);
			
			let (image, image_promise) = ImmutableImage::from_iter(data,
			                                                       ImageDimensions::Dim2d{ width, height, array_layers: 1 },
			                                                       MipmapsCount::One,
			                                                       vulkano::format::Format::R8_UNORM,
			                                                       self.queue.clone())?;
			
			let sampler = Sampler::new(self.queue.device().clone(),
			                           Filter::Nearest,
			                           Filter::Linear,
			                           MipmapMode::Nearest,
			                           SamplerAddressMode::ClampToBorder(BorderColor::FloatTransparentBlack),
			                           SamplerAddressMode::ClampToBorder(BorderColor::FloatTransparentBlack),
			                           SamplerAddressMode::ClampToBorder(BorderColor::FloatTransparentBlack),
			                           0.0,
			                           1.0,
			                           0.0,
			                           1.0)?;
			
			let set = {
				let mut set_builder = PersistentDescriptorSet::start(self.pipeline.layout().descriptor_set_layouts().get(0).unwrap().clone());
				set_builder.add_sampled_image(ImageView::new(image)?, sampler.clone())?;
				Arc::new(set_builder.build()?)
			};
			
			let entry = TextEntry {
				size: (width, height),
				set
			};
			
			drop(image_promise);
			
			self.entries.insert(text.to_string(), entry.clone());
			
			Ok(entry)
		}
	}
}

struct Rasterizer {
	glyphs: Vec<&'static Glyph>,
	x: usize,
	y: usize,
	glyph: usize,
	remaining: usize,
}

impl Rasterizer {
	fn new(glyphs: Vec<&'static Glyph>) -> Rasterizer {
		
		let width = glyphs.iter().fold(0, |acc, g| acc + g.get_width() as usize);
		let height = 16;
		
		Rasterizer {
			glyphs,
			x: 0,
			y: 0,
			glyph: 0,
			remaining: width * height,
		}
	}
}


impl Iterator for Rasterizer {
	type Item = u8;
	
	fn next(&mut self) -> Option<Self::Item> {
		if self.remaining <= 0 { return None }
		
		let glyph = self.glyphs[self.glyph];
		let color = if glyph.get_pixel(self.x, self.y) { 255 } else { 0 };
		
		self.x += 1;
		if self.x >= glyph.get_width() {
			self.x = 0;
			self.glyph += 1;
			if self.glyph >= self.glyphs.len() {
				self.glyph = 0;
				self.y += 1;
			}
		}
		
		self.remaining -= 1;
		
		Some(color)
	}
	
	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.remaining, Some(self.remaining))
	}
}

impl ExactSizeIterator for Rasterizer {
	fn len(&self) -> usize {
		self.remaining
	}
}


#[derive(Clone)]
pub struct TextEntry {
	pub size: (u32, u32),
	pub set: Arc<dyn DescriptorSet + Send + Sync>,
}

#[derive(Debug, Error)]
pub enum TextCacheError {
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
}

#[derive(Debug, Error)]
pub enum TextCacheGetError {
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] vulkano::image::ImageCreationError),
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] PersistentDescriptorSetError(#[error(source)] descriptor_set::DescriptorSetError),
	#[error(display = "{}", _0)] SamplerCreationError(#[error(source)] sampler::SamplerCreationError),
}
