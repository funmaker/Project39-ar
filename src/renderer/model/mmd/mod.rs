use std::sync::Arc;
use std::ops::Range;
use std::io::Cursor;
use cgmath::Matrix4;
use image::{DynamicImage, GenericImageView, ImageFormat};
use vulkano::buffer::{ImmutableBuffer, BufferUsage, BufferAccess};
use vulkano::image::{ImmutableImage, Dimensions};
use vulkano::sync::GpuFuture;
use vulkano::descriptor::{DescriptorSet, PipelineLayoutAbstract};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState};
use vulkano::format::Format;
use vulkano::sampler::Sampler;

mod vertex;
mod pipeline;

pub use vertex::Vertex;
use super::{Model, ModelError, VertexIndex, FenceCheck};
use crate::utils::ImageEx;
use crate::renderer::{Renderer, RenderError};
use pipeline::Pipeline;

pub struct MaterialInfo {
	pub color: [f32; 4],
	pub specular: [f32; 3],
	pub specularity: f32,
	pub ambient: [f32; 3],
	pub sphere_mode: u32,
}

struct SubMesh {
	pipeline: Pipeline,
	set: Arc<dyn DescriptorSet + Send + Sync>,
	range: Range<usize>,
}

pub struct MMDModel<VI: VertexIndex> {
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	indices: Arc<ImmutableBuffer<[VI]>>,
	sub_mesh: Vec<SubMesh>,
	fences: Vec<FenceCheck>,
	default_tex: Option<Arc<ImmutableImage<Format>>>,
}

impl<VI: VertexIndex> MMDModel<VI> {
	pub fn new(vertices: &[Vertex], indices: &[VI], renderer: &mut Renderer) -> Result<MMDModel<VI>, ModelError> {
		let queue = &renderer.load_queue;
		
		let (vertices, vertices_promise) = ImmutableBuffer::from_iter(vertices.iter().cloned(),
		                                                              BufferUsage{ vertex_buffer: true, ..BufferUsage::none() },
		                                                              queue.clone())?;
		
		let (indices, indices_promise) = ImmutableBuffer::from_iter(indices.iter().copied(),
		                                                            BufferUsage{ index_buffer: true, ..BufferUsage::none() },
		                                                            queue.clone())?;
		
		let fences = vec![FenceCheck::new(vertices_promise.join(indices_promise))?];
		
		Ok(MMDModel {
			vertices,
			indices,
			sub_mesh: vec![],
			fences,
			default_tex: None,
		})
	}
	
	pub fn add_texture(&mut self, source_image: DynamicImage, renderer: &mut Renderer) -> Result<Arc<ImmutableImage<Format>>, ModelError> {
		let queue = &renderer.load_queue;
		let width = source_image.width();
		let height = source_image.height();
		
		let (image, image_promise) = ImmutableImage::from_iter(source_image.into_pre_mul_iter(),
		                                                       Dimensions::Dim2d{ width, height },
		                                                       Format::R8G8B8A8Unorm,
		                                                       queue.clone())?;
		
		self.fences.push(FenceCheck::new(image_promise)?);
		
		Ok(image)
	}
	
	pub fn add_sub_mesh(&mut self,
	                    range: Range<usize>,
	                    material: MaterialInfo,
	                    texture: Option<Arc<ImmutableImage<Format>>>,
	                    toon: Option<Arc<ImmutableImage<Format>>>,
	                    sphere_map: Option<Arc<ImmutableImage<Format>>>,
	                    no_cull: bool,
	                    renderer: &mut Renderer)
	                    -> Result<(), ModelError> {
		let queue = &renderer.load_queue;
		
		let sampler = Sampler::simple_repeat_linear_no_mipmap(queue.device().clone());
		
		let texture = texture.map(Ok).unwrap_or_else(|| self.get_default_tex(renderer))?;
		let toon = toon.map(Ok).unwrap_or_else(|| self.get_default_tex(renderer))?;
		let sphere_map = sphere_map.map(Ok).unwrap_or_else(|| self.get_default_tex(renderer))?;
		
		let (material_buffer, material_promise) = ImmutableBuffer::from_data(material,
		                                                                     BufferUsage{ uniform_buffer: true, ..BufferUsage::none() },
		                                                                     renderer.load_queue.clone())?;
		
		let pipeline = Pipeline::get(renderer, no_cull)?;
		
		let set = Arc::new(
			PersistentDescriptorSet::start(pipeline.as_abstract()
			                                       .descriptor_set_layout(0)
			                                       .ok_or(ModelError::NoLayout)?
			                                       .clone())
				.add_buffer(renderer.commons.clone())?
				.add_buffer(material_buffer)?
				.add_sampled_image(texture, sampler.clone())?
				.add_sampled_image(toon, sampler.clone())?
				.add_sampled_image(sphere_map, sampler.clone())?
				.build()?
		);
		
		self.sub_mesh.push(SubMesh{ pipeline, set, range });
		
		self.fences.push(FenceCheck::new(material_promise)?);
		
		Ok(())
	}
	
	fn get_default_tex(&mut self, renderer: &mut Renderer) -> Result<Arc<ImmutableImage<Format>>, ModelError> {
		if let Some(image) = self.default_tex.clone() {
			return Ok(image);
		}
		
		let texture_reader = Cursor::new(include_bytes!("./default_tex.png"));
		let image = image::load(texture_reader, ImageFormat::Png)?;
		let texture = self.add_texture(image, renderer)?;
		
		self.default_tex = Some(texture.clone());
		
		Ok(texture)
	}
	
	pub fn loaded(&self) -> bool {
		self.fences.iter().all(|fence| fence.check())
	}
}

impl<VI: VertexIndex> Model for MMDModel<VI> {
	fn render(&self, builder: &mut AutoCommandBufferBuilder, model_matrix: Matrix4<f32>, eye: u32) -> Result<(), RenderError> {
		if !self.loaded() { return Ok(()) }
		
		for sub_mesh in self.sub_mesh.iter() {
			match sub_mesh.pipeline.clone() {
				Pipeline::Cull(pipeline) => {
					builder.draw_indexed(pipeline,
					                     &DynamicState::none(),
					                     self.vertices.clone(),
					                     self.indices.clone().into_buffer_slice().slice(sub_mesh.range.clone()).unwrap(),
					                     sub_mesh.set.clone(),
					                     (model_matrix, eye))?;
				}
				Pipeline::NoCull(pipeline) => {
					builder.draw_indexed(pipeline,
					                     &DynamicState::none(),
					                     self.vertices.clone(),
					                     self.indices.clone().into_buffer_slice().slice(sub_mesh.range.clone()).unwrap(),
					                     sub_mesh.set.clone(),
					                     (model_matrix, eye))?;
				}
			}
		}
		
		Ok(())
	}
}


