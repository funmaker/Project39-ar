use std::ops::Range;
use std::hash::Hash;
use std::fmt::Debug;
use bytemuck::Pod;
use err_derive::Error;
use vulkano::{command_buffer, descriptor_set, DeviceSize, memory, sampler, sync};
use vulkano::pipeline::graphics::input_assembly::Index;

pub mod simple;
pub mod mmd;
pub mod gimp;
pub mod billboard;

use crate::renderer::pipelines::PipelineError;
use crate::renderer::assets_manager::{AssetError, TextureLoadError};
pub use simple::SimpleModel;
pub use self::mmd::MMDModel;
pub use billboard::Billboard;

pub trait VertexIndex: Index + Pod + Copy + Send + Sync + Sized + Into<u32> + Hash + Debug + 'static {}
impl<T> VertexIndex for T where T: Index + Pod + Copy + Send + Sync + Sized + Into<u32> + Hash + Debug + 'static {}

#[derive(Debug, Error)]
pub enum ModelError {
	#[error(display = "Pipeline doesn't have specified layout")] NoLayout,
	#[error(display = "Invalid indices range: {:?}, len: {}", _0, _1)] IndicesRangeError(Range<DeviceSize>, DeviceSize),
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] AssetError(#[error(source)] AssetError),
	#[error(display = "{}", _0)] TextureLoadError(#[error(source)] TextureLoadError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] AllocationCreationError(#[error(source)] memory::allocator::AllocationCreationError),
	#[error(display = "{}", _0)] ImmutableImageCreationError(#[error(source)] vulkano::image::immutable::ImmutableImageCreationError),
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] DescriptorSetCreationError(#[error(source)] descriptor_set::DescriptorSetCreationError),
	#[error(display = "{}", _0)] SamplerCreationError(#[error(source)] sampler::SamplerCreationError),
	#[error(display = "{}", _0)] CommandBufferBeginError(#[error(source)] command_buffer::CommandBufferBeginError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] command_buffer::CommandBufferExecError),
	#[error(display = "{}", _0)] BuildError(#[error(source)] command_buffer::BuildError),
}
