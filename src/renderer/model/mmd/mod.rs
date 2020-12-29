use std::sync::Arc;
use std::ops::Range;
use cgmath::Matrix4;
use image::{DynamicImage, GenericImageView};
use vulkano::buffer::{ImmutableBuffer, BufferUsage, BufferAccess};
use vulkano::image::{ImmutableImage, Dimensions};
use vulkano::sync::GpuFuture;
use vulkano::descriptor::{DescriptorSet, PipelineLayoutAbstract};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState};
use vulkano::format::Format;
use vulkano::sampler::Sampler;

mod vertex;

pub use vertex::Vertex;
use crate::renderer::{Renderer, RenderError};
use crate::renderer::pipelines::MMDPipeline;
use super::{Model, ModelError, VertexIndex, FenceCheck};

struct SubMesh {
	set: Arc<dyn DescriptorSet + Send + Sync>,
	range: Range<usize>,
	fence: FenceCheck,
}

pub struct MMDModel<VI: VertexIndex> {
	pipeline: Arc<MMDPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	indices: Arc<ImmutableBuffer<[VI]>>,
	sub_mesh: Vec<SubMesh>,
	fence: FenceCheck,
}

impl<VI: VertexIndex> MMDModel<VI> {
	pub fn new(vertices: &[Vertex], indices: &[VI], renderer: &mut Renderer) -> Result<MMDModel<VI>, ModelError> {
		let queue = &renderer.load_queue;
		
		let pipeline = renderer.pipelines.get::<MMDPipeline>()?;
		
		let (vertices, vertices_promise) = ImmutableBuffer::from_iter(vertices.iter().cloned(),
		                                                              BufferUsage{ vertex_buffer: true, ..BufferUsage::none() },
		                                                              queue.clone())?;
		
		let (indices, indices_promise) = ImmutableBuffer::from_iter(indices.iter().copied(),
		                                                            BufferUsage{ index_buffer: true, ..BufferUsage::none() },
		                                                            queue.clone())?;
		
		let fence = FenceCheck::new(vertices_promise.join(indices_promise))?;
		
		Ok(MMDModel {
			pipeline,
			vertices,
			indices,
			sub_mesh: vec![],
			fence,
		})
	}
	
	pub fn add_sub_mesh(&mut self, range: Range<usize>, source_image: DynamicImage, renderer: &mut Renderer) -> Result<(), ModelError> {
		let width = source_image.width();
		let height = source_image.height();
		let queue = &renderer.load_queue;
		
		let (image, image_promise) = ImmutableImage::from_iter(source_image.to_rgba8().into_vec().into_iter(),
		                                                       Dimensions::Dim2d{ width, height },
		                                                       Format::R8G8B8A8Unorm,
		                                                       queue.clone())?;
		
		let sampler = Sampler::simple_repeat_linear_no_mipmap(queue.device().clone());
		
		let set = Arc::new(
			PersistentDescriptorSet::start(self.pipeline.descriptor_set_layout(0).ok_or(ModelError::NoLayout)?.clone())
				.add_sampled_image(image.clone(), sampler.clone())?
				.build()?
		);
		
		let fence = FenceCheck::new(image_promise)?;
		
		self.sub_mesh.push(SubMesh{ set, range, fence });
		
		Ok(())
	}
	
	pub fn loaded(&self) -> bool {
		self.fence.check() && self.sub_mesh.iter().all(|sm| sm.fence.check())
	}
}

impl<VI: VertexIndex> Model for MMDModel<VI> {
	fn render(&self, builder: &mut AutoCommandBufferBuilder, pvm_matrix: Matrix4<f32>) -> Result<(), RenderError> {
		if !self.loaded() { return Ok(()) }
		
		for sub_mesh in self.sub_mesh.iter() {
			builder.draw_indexed(self.pipeline.clone(),
			                     &DynamicState::none(),
			                     self.vertices.clone(),
			                     self.indices.clone().into_buffer_slice().slice(sub_mesh.range.clone()).unwrap(),
			                     sub_mesh.set.clone(),
			                     pvm_matrix)?;
		}
		
		Ok(())
	}
}


