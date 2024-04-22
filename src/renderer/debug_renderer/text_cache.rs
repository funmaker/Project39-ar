use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use unifont::Glyph;
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract};
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::Queue;
use vulkano::image::{ImmutableImage, MipmapsCount, ImageDimensions};
use vulkano::image::view::ImageView;
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::pipeline::{Pipeline, GraphicsPipeline};
use vulkano::sampler::{Sampler, Filter, SamplerAddressMode, BorderColor, SamplerCreateInfo, SamplerMipmapMode};

use super::super::pipelines::Pipelines;
use super::super::pipelines::debug::DebugTexturedPipeline;


pub struct TextCache {
	pipeline: Arc<GraphicsPipeline>,
	queue: Arc<Queue>,
	memory_allocator: Arc<StandardMemoryAllocator>,
	command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
	descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
	entries: HashMap<String, TextEntry>,
	max_stale: usize,
}

const TARGET_SIZE: usize = 512;

// Unifont's U+FFFD � REPLACEMENT CHARACTER
const REPLACEMENT_GLYPH: Glyph = Glyph::HalfWidth([
	0x00, 0x00, 0x00, 0x7E, 0x66, 0x5A, 0x5A, 0x7A, 0x76, 0x76, 0x7E, 0x76, 0x76, 0x7E, 0x00, 0x00
]);

impl TextCache {
	pub fn new(queue: &Arc<Queue>, memory_allocator: &Arc<StandardMemoryAllocator>, command_buffer_allocator: &Arc<StandardCommandBufferAllocator>, descriptor_set_allocator: &Arc<StandardDescriptorSetAllocator>, pipelines: &mut Pipelines) -> Result<Self> {
		let pipeline = pipelines.get::<DebugTexturedPipeline>()?;
		
		Ok(TextCache {
			pipeline,
			queue: queue.clone(),
			memory_allocator: memory_allocator.clone(),
			command_buffer_allocator: command_buffer_allocator.clone(),
			descriptor_set_allocator: descriptor_set_allocator.clone(),
			entries: HashMap::new(),
			max_stale: 1024,
		})
	}
	
	pub fn get(&mut self, text: &'_ str) -> Result<TextEntry> {
		if let Some(entry) = self.entries.get_mut(text) {
			entry.stale = 0;
			Ok(entry.clone())
		} else {
			let glyphs = text.chars()
			                 .map(|c| unifont::get_glyph(c).unwrap_or(&REPLACEMENT_GLYPH))
			                 .collect::<Vec<_>>();
			
			let width = glyphs.iter().fold(0, |acc, g| acc + g.get_width() as u32);
			let height = 16;
			
			let data = Rasterizer::new(glyphs);
			
			let mut upload_buffer = AutoCommandBufferBuilder::primary(&*self.command_buffer_allocator,
			                                                          self.queue.queue_family_index(),
			                                                          CommandBufferUsage::OneTimeSubmit)?;
			
			let image = ImmutableImage::from_iter(&self.memory_allocator,
			                                      data,
			                                      ImageDimensions::Dim2d{ width, height, array_layers: 1 },
			                                      MipmapsCount::One,
			                                      vulkano::format::Format::R8_UNORM,
			                                      &mut upload_buffer)?;
			
			let sampler = Sampler::new(self.queue.device().clone(), SamplerCreateInfo {
				mag_filter: Filter::Nearest,
				min_filter: Filter::Linear,
				mipmap_mode: SamplerMipmapMode::Nearest,
				address_mode: [SamplerAddressMode::ClampToBorder; 3],
				border_color: BorderColor::FloatTransparentBlack,
				..SamplerCreateInfo::default()
			})?;
			
			let set = PersistentDescriptorSet::new(&self.descriptor_set_allocator,
			                                       self.pipeline.layout().set_layouts().get(0).unwrap().clone(), [
				                                       WriteDescriptorSet::image_view_sampler(0, ImageView::new_default(image)?, sampler.clone()),
			                                       ])?;
			
			let entry = TextEntry {
				size: (width, height),
				set,
				stale: 0,
			};
			
			let upload_future = upload_buffer.build()?
			                                 .execute(self.queue.clone())?;
			
			drop(upload_future);
			
			self.entries.insert(text.to_string(), entry.clone());
			
			Ok(entry)
		}
	}
	
	pub fn cleanup(&mut self) {
		for entry in self.entries.values_mut() {
			entry.stale += 1;
		}
		
		let max_stale = self.max_stale;
		self.entries.retain(|_, e| e.stale <= max_stale);
		
		if self.entries.len() > TARGET_SIZE && self.max_stale > 2 {
			self.max_stale /= 2;
		}
		
		if self.entries.len() < TARGET_SIZE / 2 && self.max_stale < 1024 {
			self.max_stale *= 2;
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
	pub set: Arc<PersistentDescriptorSet>,
	pub stale: usize,
}

