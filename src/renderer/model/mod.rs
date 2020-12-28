use std::sync::{Arc, RwLock, PoisonError};
use std::time::Duration;
use std::ops::Deref;
use err_derive::Error;
use cgmath::Matrix4;
use image::DynamicImage;
use vulkano::buffer::{ImmutableBuffer, BufferUsage};
use vulkano::image::ImageCreationError;
use vulkano::sync::{GpuFuture, FlushError, FenceSignalFuture};
use vulkano::memory::DeviceMemoryAllocError;
use vulkano::descriptor::descriptor_set::{PersistentDescriptorSetError, PersistentDescriptorSetBuildError};
use vulkano::command_buffer::AutoCommandBufferBuilder;

pub mod vertex;
pub mod import;
pub mod sub_mesh;

pub use vertex::Vertex;
pub use import::{LoadError, from_obj, from_openvr, from_pmx};
pub use sub_mesh::SubMesh;
use super::{Renderer, PipelineType, RenderError};

pub type VertexIndex = u16;

pub struct Model {
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	sub_meshes: Vec<SubMesh>,
	fence: Arc<RwLock<FenceCheck>>,
}

impl Model {
	pub fn new(vertices: &[Vertex], renderer: &Renderer) -> Result<Model, ModelError> {
		let queue = &renderer.load_queue;
		
		let (vertices, vertices_promise) = ImmutableBuffer::from_iter(vertices.iter().cloned(),
		                                                              BufferUsage{ vertex_buffer: true, ..BufferUsage::none() },
		                                                              queue.clone())?;
		
		let fence = Arc::new(RwLock::new(FenceCheck::new(vertices_promise)?));
		
		Ok(Model {
			vertices,
			sub_meshes: Vec::new(),
			fence,
		})
	}
	
	pub fn simple(vertices: &[Vertex], indices: &[VertexIndex], source_image: DynamicImage, renderer: &Renderer) -> Result<Model, ModelError> {
		let mut model = Model::new(vertices, renderer)?;
		
		model.add_sub_mesh(indices, source_image, renderer)?;
		
		Ok(model)
	}
	
	pub fn add_sub_mesh(&mut self, indices: &[VertexIndex], source_image: DynamicImage, renderer: &Renderer) -> Result<(), ModelError> {
		let (mesh, mesh_promise) = SubMesh::new(indices, source_image, renderer)?;
		
		self.sub_meshes.push(mesh);
		
		self.fence.write()?
		          .append(mesh_promise)?;
		
		Ok(())
	}
	
	pub fn loaded(&self) -> bool {
		let result;
		
		match self.fence.read().as_ref().map(Deref::deref) {
			// TODO: propagate Errors
			Err(_) => return false,
			Ok(&FenceCheck::Done(ref result)) => return *result,
			Ok(&FenceCheck::Pending(ref fence)) => {
				match fence.wait(Some(Duration::new(0, 0))) {
					Err(FlushError::Timeout) => return false,
					Ok(()) => result = true,
					Err(err) => {
						eprintln!("Error while loading renderer.model: {:?}", err);
						result = false;
					}
				}
			}
		}
		
		if let Ok(mut fence) = self.fence.write() {
			*fence = FenceCheck::Done(result);
		}
		
		result
	}
	
	pub fn render(&self, builder: &mut AutoCommandBufferBuilder, pipeline: &Arc<PipelineType>, pvm_matrix: Matrix4<f32>) -> Result<(), RenderError> {
		if self.loaded() {
			for sub_mesh in self.sub_meshes.iter() {
				sub_mesh.render(self, builder, pipeline, pvm_matrix)?;
			}
		}
		
		Ok(())
	}
}

enum FenceCheck {
	Done(bool),
	Pending(FenceSignalFuture<Box<dyn GpuFuture>>)
}

impl FenceCheck {
	fn new<GF>(future: GF)
	          -> Result<FenceCheck, FlushError>
	          where GF: GpuFuture + 'static {
		Ok(FenceCheck::Pending((Box::new(future) as Box<dyn GpuFuture>).then_signal_fence_and_flush()?))
	}
	
	fn append<GF>(&mut self, other: GF)
	             -> Result<(), FlushError>
	             where GF: GpuFuture + 'static {
		let this = std::mem::replace(self, FenceCheck::Done(false));
		
		*self = match this {
			FenceCheck::Done(false) => FenceCheck::Done(false),
			FenceCheck::Done(true) => FenceCheck::Pending((Box::new(other) as Box<dyn GpuFuture>).then_signal_fence_and_flush()?),
			FenceCheck::Pending(gpu_future) => FenceCheck::Pending((Box::new(gpu_future.join(other)) as Box<dyn GpuFuture>).then_signal_fence_and_flush()?),
		};
		
		Ok(())
	}
}


#[derive(Debug, Error)]
pub enum ModelError {
	#[error(display = "Pipeline doesn't have layout set 0")] NoLayout,
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] DeviceMemoryAllocError),
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] ImageCreationError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] FlushError),
	#[error(display = "{}", _0)] PersistentDescriptorSetError(#[error(source)] PersistentDescriptorSetError),
	#[error(display = "{}", _0)] PersistentDescriptorSetBuildError(#[error(source)] PersistentDescriptorSetBuildError),
	#[error(display = "{}", _0)] PoisonError(String),
}

impl<S> From<PoisonError<S>> for ModelError {
	fn from(poison: PoisonError<S>) -> Self {
		ModelError::PoisonError(poison.to_string())
	}
}
