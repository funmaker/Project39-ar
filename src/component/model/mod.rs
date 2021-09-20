use std::ops::Range;
use err_derive::Error;
use vulkano::{descriptor_set, DeviceSize, memory, sync};
use vulkano::pipeline::input_assembly::Index;

pub mod simple;
pub mod mmd;

use crate::renderer::pipelines::PipelineError;
pub use simple::SimpleModel;
pub use self::mmd::MMDModel;

pub trait VertexIndex: Index + Copy + Send + Sync + Sized + Into<i32> + 'static {}
impl<T> VertexIndex for T where T: Index + Copy + Send + Sync + Sized + Into<i32> + 'static {}

#[derive(Debug, Error)]
pub enum ModelError {
	#[error(display = "Pipeline doesn't have specified layout")] NoLayout,
	#[error(display = "Invalid indices range: {:?}, len: {}", _0, _1)] IndicesRangeError(Range<DeviceSize>, DeviceSize),
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] vulkano::image::ImageCreationError),
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] DescriptorSetError(#[error(source)] descriptor_set::DescriptorSetError),
}