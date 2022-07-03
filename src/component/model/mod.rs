use std::ops::Range;
use std::hash::Hash;
use std::fmt::Debug;
use bytemuck::Pod;
use err_derive::Error;
use vulkano::{descriptor_set, DeviceSize, memory, sampler, sync};
use vulkano::pipeline::graphics::input_assembly::Index;

pub mod simple;
pub mod mmd;

use crate::renderer::pipelines::PipelineError;
pub use simple::SimpleModel;
pub use self::mmd::MMDModel;

pub trait VertexIndex: Index + Pod + Copy + Send + Sync + Sized + Into<u32> + Hash + Debug + 'static {}
impl<T> VertexIndex for T where T: Index + Pod + Copy + Send + Sync + Sized + Into<u32> + Hash + Debug + 'static {}

#[derive(Debug, Error)]
pub enum ModelError {
	#[error(display = "Pipeline doesn't have specified layout")] NoLayout,
	#[error(display = "Invalid indices range: {:?}, len: {}", _0, _1)] IndicesRangeError(Range<DeviceSize>, DeviceSize),
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] DeviceMemoryAllocationError(#[error(source)] memory::DeviceMemoryAllocationError),
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] vulkano::image::ImageCreationError),
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] DescriptorSetCreationError(#[error(source)] descriptor_set::DescriptorSetCreationError),
	#[error(display = "{}", _0)] SamplerCreationError(#[error(source)] sampler::SamplerCreationError),
}
