use std::sync::Arc;
use std::ops::Range;
use std::io::Cursor;
use std::convert::TryFrom;
use std::cell::RefCell;
use image::{DynamicImage, GenericImageView, ImageFormat};
use vulkano::buffer::{ImmutableBuffer, BufferUsage, BufferAccess, CpuBufferPool};
use vulkano::image::{ImmutableImage, Dimensions, MipmapsCount};
use vulkano::sync::GpuFuture;
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState};
use vulkano::format::Format;

mod sub_mesh;
mod import;
pub mod test;

use super::{Model, ModelError, VertexIndex, FenceCheck};
use crate::application::entity::{Bone, BoneConnection};
use crate::renderer::{Renderer, RendererRenderError};
use crate::utils::ImageEx;
use crate::debug;
use crate::math::{AMat4, ToTransform};
pub use crate::renderer::pipelines::mmd::Vertex;
pub use sub_mesh::MaterialInfo;
pub use import::MMDModelLoadError;
use sub_mesh::SubMesh;

pub struct MMDModel<VI: VertexIndex> {
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	indices: Arc<ImmutableBuffer<[VI]>>,
	sub_mesh: Vec<SubMesh>,
	fences: Vec<FenceCheck>,
	default_tex: Option<Arc<ImmutableImage<Format>>>,
	default_bones: Vec<Bone>,
	bones_ubo: RefCell<Vec<AMat4>>,
	bones_pool: CpuBufferPool<AMat4>,
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
		
		let bones_pool = CpuBufferPool::uniform_buffer(queue.device().clone());
		
		Ok(MMDModel {
			vertices,
			indices,
			sub_mesh: vec![],
			fences,
			default_tex: None,
			default_bones: vec![],
			bones_ubo: RefCell::new(vec![]),
			bones_pool,
		})
	}
	
	pub fn from_pmx(path: &str, renderer: &mut Renderer) -> Result<MMDModel<VI>, MMDModelLoadError> where VI: mmd::VertexIndex {
		import::from_pmx(path, renderer)
	}
	
	pub fn add_texture(&mut self, source_image: DynamicImage, renderer: &mut Renderer) -> Result<Arc<ImmutableImage<Format>>, ModelError> {
		let queue = &renderer.load_queue;
		let width = source_image.width();
		let height = source_image.height();
		
		let (image, image_promise) = ImmutableImage::from_iter(source_image.into_pre_mul_iter(),
		                                                       Dimensions::Dim2d{ width, height },
		                                                       MipmapsCount::Log2,
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
	                    opaque: bool,
	                    edge: Option<(f32, [f32; 4])>,
	                    renderer: &mut Renderer)
	                    -> Result<(), ModelError> {
		let texture = texture.map(Ok).unwrap_or_else(|| self.get_default_tex(renderer))?;
		let toon = toon.map(Ok).unwrap_or_else(|| self.get_default_tex(renderer))?;
		let sphere_map = sphere_map.map(Ok).unwrap_or_else(|| self.get_default_tex(renderer))?;
		
		let (material_buffer, material_promise) = ImmutableBuffer::from_data(material,
		                                                                     BufferUsage{ uniform_buffer: true, ..BufferUsage::none() },
		                                                                     renderer.load_queue.clone())?;
		
		let sub_mesh = SubMesh::new(range, material_buffer, texture, toon, sphere_map, opaque, no_cull, edge, renderer)?;
		
		self.sub_mesh.push(sub_mesh);
		
		self.fences.push(FenceCheck::new(material_promise)?);
		
		Ok(())
	}
	
	fn get_default_tex(&mut self, renderer: &mut Renderer) -> Result<Arc<ImmutableImage<Format>>, ModelError> {
		if let Some(image) = self.default_tex.clone() {
			return Ok(image);
		}
		
		let texture_reader = Cursor::new(&include_bytes!("./default_tex.png")[..]);
		let image = image::load(texture_reader, ImageFormat::Png)?;
		let texture = self.add_texture(image, renderer)?;
		
		self.default_tex = Some(texture.clone());
		
		Ok(texture)
	}
	
	fn add_bone(&mut self, bone: Bone) {
		self.default_bones.push(bone);
	}
	
	fn draw_debug_bones(&self, model_matrix: &AMat4, bones: &Vec<Bone>, bones_mats: &Vec<AMat4>) {
		for (id, bone) in bones.iter().enumerate() {
			if bone.display {
				let pos = model_matrix.transform_point(&bones_mats[id].transform_point(&bone.rest_pos()));
				
				debug::draw_point(&pos, 10.0, bone.color.clone());
				debug::draw_text(&bone.name, &pos, debug::DebugOffset::bottom_right(8.0, 8.0), 32.0, bone.color.clone());
				
				match &bone.connection {
					BoneConnection::None => {}
					BoneConnection::Bone(con) => {
						let cpos = model_matrix.transform_point(&bones_mats[*con].transform_point(&bones[*con].rest_pos()));
						debug::draw_line(pos, cpos, 3.0, bone.color.clone());
					}
					BoneConnection::Offset(cpos) => {
						let cpos = model_matrix.transform_point(&bones_mats[id].transform_point(&(&bone.rest_pos() + cpos)));
						debug::draw_line(pos, cpos, 3.0, bone.color.clone());
					}
				}
			}
		}
	}
	
	pub fn loaded(&self) -> bool {
		self.fences.iter().all(|fence| fence.check())
	}
}

impl<VI: VertexIndex> Model for MMDModel<VI> {
	fn render(&self, builder: &mut AutoCommandBufferBuilder, model_matrix: &AMat4, eye: u32, bones: &Vec<Bone>) -> Result<(), RendererRenderError> {
		if !self.loaded() { return Ok(()) }
		
		let buffer = {
			let mut bones_mats = self.bones_ubo.borrow_mut();
			bones_mats.reserve(bones.len());
			
			for bone in bones {
				let transform = match bone.parent {
					None => &bone.local_transform * &bone.anim_transform.to_transform(),
					Some(id) => &bones_mats[id] * &bone.local_transform * &bone.anim_transform,
				};
				
				bones_mats.push(transform);
			}
			
			for (id, mat) in bones_mats.iter_mut().enumerate() {
				*mat = *mat * &bones[id].inv_model_transform;
			}
			
			if debug::get_flag_or_default("KeyB") {
				self.draw_debug_bones(&model_matrix, &bones, &bones_mats);
			}
			
			self.bones_pool.chunk(bones_mats.drain(..)) // TODO: There has to be better way
		}?;
		
		// Outline
		for sub_mesh in self.sub_mesh.iter() {
			if let Some((pipeline, set, pool)) = sub_mesh.edge.clone() {
				let index_buffer = self.indices.clone().into_buffer_slice().slice(sub_mesh.range.clone()).unwrap();
		
				// calculate size of one pixel at distance 1m from camera
				// Assume index
				// 1440×1600 110 FOV
				let pixel = (110.0_f32 / 360.0 * std::f32::consts::PI).tan() * 2.0 / 1440.0;
				let scale: f32 = pixel * sub_mesh.edge_scale;
				
				let bones_set = pool.borrow_mut().next().add_buffer(buffer.clone())?.build()?;
				
				builder.draw_indexed(pipeline,
				                     &DynamicState::none(),
				                     self.vertices.clone(),
				                     index_buffer.clone(),
				                     (set, bones_set),
				                     (model_matrix.to_homogeneous(), sub_mesh.edge_color, eye, scale))?;
			}
		}
		
		// Opaque
		for sub_mesh in self.sub_mesh.iter() {
			let index_buffer = self.indices.clone().into_buffer_slice().slice(sub_mesh.range.clone()).unwrap();
			let (pipeline, set, pool) = sub_mesh.main.clone();
			
			let bones_set = pool.borrow_mut().next().add_buffer(buffer.clone())?.build()?;
		
			builder.draw_indexed(pipeline,
			                     &DynamicState::none(),
			                     self.vertices.clone(),
			                     index_buffer.clone(),
			                     (set, bones_set),
			                     (model_matrix.to_homogeneous(), eye))?;
		}
		
		// Transparent
		for sub_mesh in self.sub_mesh.iter() {
			if let Some((pipeline, set, pool)) = sub_mesh.transparent.clone() {
				let index_buffer = self.indices.clone().into_buffer_slice().slice(sub_mesh.range.clone()).unwrap();
				
				let bones_set = pool.borrow_mut().next().add_buffer(buffer.clone())?.build()?;
		
				builder.draw_indexed(pipeline,
				                     &DynamicState::none(),
				                     self.vertices.clone(),
				                     index_buffer.clone(),
				                     (set, bones_set),
				                     (model_matrix.to_homogeneous(), eye))?;
			}
		}
		
		Ok(())
	}
	
	fn get_default_bones(&self) -> Vec<Bone> {
		self.default_bones.clone()
	}
}
