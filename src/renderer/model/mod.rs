use err_derive::Error;
use vulkano::{memory, sync, command_buffer};
use vulkano::descriptor::descriptor_set;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::pipeline::input_assembly::Index;

pub mod simple;
pub mod mmd;
mod fence_check;

use crate::application::entity::Bone;
use super::pipelines::PipelineError;
use crate::renderer::Renderer;
use crate::math::AMat4;
pub use self::mmd::MMDModel;
pub use simple::SimpleModel;
pub use fence_check::FenceCheck;

pub trait Model {
	#[allow(unused_variables)]
	fn pre_render(&mut self, builder: &mut AutoCommandBufferBuilder, model_matrix: &AMat4, bones: &[Bone], morphs: &[f32]) -> Result<(), ModelRenderError> { Ok(()) }
	fn render(&mut self, builder: &mut AutoCommandBufferBuilder, model_matrix: &AMat4, eye: u32) -> Result<(), ModelRenderError>;
	fn get_default_bones(&self) -> &[Bone] { &[] }
	fn morphs_count(&self) -> usize { 0 }
	fn try_clone(&self, renderer: &mut Renderer) -> Result<Box<dyn Model>, ModelError>;
}

pub trait VertexIndex: Index + Copy + Send + Sync + Sized + Into<i32> + 'static {}
impl<T> VertexIndex for T where T: Index + Copy + Send + Sync + Sized + Into<i32> + 'static {}

#[derive(Debug, Error)]
pub enum ModelError {
	#[error(display = "Pipeline doesn't have specified layout")] NoLayout,
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] vulkano::image::ImageCreationError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] PersistentDescriptorSetError(#[error(source)] descriptor_set::PersistentDescriptorSetError),
	#[error(display = "{}", _0)] PersistentDescriptorSetBuildError(#[error(source)] descriptor_set::PersistentDescriptorSetBuildError),
}

#[derive(Debug, Error)]
pub enum ModelRenderError {
	#[error(display = "{}", _0)] DrawIndexedError(#[error(source)] command_buffer::DrawIndexedError),
	#[error(display = "{}", _0)] CopyBufferError(#[error(source)] command_buffer::CopyBufferError),
	#[error(display = "{}", _0)] FillBufferError(#[error(source)] command_buffer::FillBufferError),
	#[error(display = "{}", _0)] DispatchError(#[error(source)] command_buffer::DispatchError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
}
