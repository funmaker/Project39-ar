use cgmath::Matrix4;
use err_derive::Error;
use vulkano::{memory, sync};
use vulkano::descriptor::descriptor_set;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::pipeline::input_assembly::Index;

pub mod simple;
pub mod mmd;
mod import;
mod fence_check;

use super::RenderError;
use super::pipelines::PipelineError;
pub use simple::SimpleModel;
pub use import::*;
pub use fence_check::FenceCheck;

pub trait Model {
	fn render(&self, builder: &mut AutoCommandBufferBuilder, model_matrix: Matrix4<f32>, eye: u32) -> Result<(), RenderError>;
}

pub trait VertexIndex: Index + Copy + Send + Sync + Sized + 'static {}
impl<T> VertexIndex for T where T: Index + Copy + Send + Sync + Sized + 'static {}

#[derive(Debug, Error)]
pub enum ModelError {
	#[error(display = "Pipeline doesn't have layout set 0")] NoLayout,
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] vulkano::image::ImageCreationError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] PersistentDescriptorSetError(#[error(source)] descriptor_set::PersistentDescriptorSetError),
	#[error(display = "{}", _0)] PersistentDescriptorSetBuildError(#[error(source)] descriptor_set::PersistentDescriptorSetBuildError),
}
