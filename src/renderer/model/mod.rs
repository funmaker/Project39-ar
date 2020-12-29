use cgmath::Matrix4;
use err_derive::Error;
use vulkano::memory::DeviceMemoryAllocError;
use vulkano::image::ImageCreationError;
use vulkano::sync::FlushError;
use vulkano::descriptor::descriptor_set::{PersistentDescriptorSetError, PersistentDescriptorSetBuildError};
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::pipeline::input_assembly::Index;

pub mod simple;
pub mod import;

pub use simple::SimpleModel;
pub use import::*;
use super::RenderError;
use super::pipelines::PipelineError;

pub trait Model {
	fn render(&self, builder: &mut AutoCommandBufferBuilder, pvm_matrix: Matrix4<f32>) -> Result<(), RenderError>;
}

pub trait VertexIndex: Index + Copy + Send + Sync + Sized + 'static {}
impl<T> VertexIndex for T where T: Index + Copy + Send + Sync + Sized + 'static {}

#[derive(Debug, Error)]
pub enum ModelError {
	#[error(display = "Pipeline doesn't have layout set 0")] NoLayout,
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] DeviceMemoryAllocError),
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] ImageCreationError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] FlushError),
	#[error(display = "{}", _0)] PersistentDescriptorSetError(#[error(source)] PersistentDescriptorSetError),
	#[error(display = "{}", _0)] PersistentDescriptorSetBuildError(#[error(source)] PersistentDescriptorSetBuildError),
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
}
