use std::sync::Arc;
use vulkano::command_buffer::pool::standard::StandardCommandPoolBuilder;
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState};
use vulkano::buffer::{BufferUsage, BufferAccess, DeviceLocalBuffer, TypedBufferAccess};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::descriptor::{DescriptorSet, PipelineLayoutAbstract};
use vulkano::device::DeviceOwned;

pub mod test;
mod sub_mesh;
mod import;
mod shared;

use super::{Model, ModelError, ModelRenderError, VertexIndex};
use crate::application::entity::{Bone};
use crate::renderer::Renderer;
use crate::math::{AMat4, ToTransform, IVec4};
use crate::renderer::pipelines::mmd::GROUP_SIZE;
pub use crate::renderer::pipelines::mmd::Vertex;
pub use import::MMDModelLoadError;
use shared::MMDModelShared;

pub struct MMDModel<VI: VertexIndex> {
	shared: Arc<MMDModelShared<VI>>,
	bones_mats: Vec<AMat4>,
	bones_ubo: Arc<DeviceLocalBuffer<[AMat4]>>,
	morphs_vec: Vec<IVec4>,
	morphs_ubo: Arc<DeviceLocalBuffer<[IVec4]>>,
	offsets_ubo: Arc<DeviceLocalBuffer<[IVec4]>>,
	morphs_set: Arc<dyn DescriptorSet + Send + Sync>,
	model_set: Arc<dyn DescriptorSet + Send + Sync>,
}

impl<VI: VertexIndex> MMDModel<VI> {
	fn new(shared: Arc<MMDModelShared<VI>>, renderer: &mut Renderer) -> Result<MMDModel<VI>, ModelError> {
		let bone_count = shared.default_bones.len();
		
		let bones_mats = Vec::with_capacity(shared.default_bones.len());
		let bones_ubo = DeviceLocalBuffer::array(shared.vertices.device().clone(),
		                                         bone_count,
		                                         BufferUsage {
			                                         transfer_destination: true,
			                                         uniform_buffer: true,
			                                         ..BufferUsage::none()
		                                         },
		                                         Some(renderer.queue.family()))?;
		
		let morphs_count = (shared.morphs_sizes.len() + 1) / 2;
		let morphs_ubo = DeviceLocalBuffer::array(shared.vertices.device().clone(),
		                                          morphs_count,
		                                          BufferUsage {
			                                          transfer_destination: true,
			                                          storage_buffer: true,
			                                          uniform_buffer: true,
			                                          ..BufferUsage::none()
		                                          },
		                                          Some(renderer.queue.family()))?;
		
		let offsets_ubo = DeviceLocalBuffer::array(shared.vertices.device().clone(),
		                                           shared.vertices.len(),
		                                           BufferUsage {
			                                           transfer_destination: true,
			                                           storage_buffer: true,
			                                           uniform_buffer: true,
			                                           ..BufferUsage::none()
		                                           },
		                                           Some(renderer.queue.family()))?;
		
		let layout = shared.morphs_pipeline.layout()
		                                   .descriptor_set_layout(0)
		                                   .ok_or(ModelError::NoLayout)?
		                                   .clone();
		
		let morphs_set = Arc::new(
			PersistentDescriptorSet::start(layout)
				.add_buffer(morphs_ubo.clone())?
				.add_buffer(shared.morphs_desc.as_ref().unwrap().clone())?
				.add_buffer(offsets_ubo.clone())?
				.build()?
		);
		
		let model_set = Arc::new(
			PersistentDescriptorSet::start(shared.commons_layout(renderer)?)
				.add_buffer(renderer.commons.clone())?
				.add_buffer(bones_ubo.clone())?
				.add_buffer(offsets_ubo.clone())?
				.build()?
		);
		
		Ok(MMDModel {
			bones_mats,
			bones_ubo,
			shared,
			morphs_vec: vec![],
			morphs_ubo,
			morphs_set,
			offsets_ubo,
			model_set,
		})
	}
	
	#[allow(unused)]
	pub fn from_pmx(path: &str, renderer: &mut Renderer) -> Result<MMDModel<VI>, MMDModelLoadError> where VI: mmd::VertexIndex {
		let shared = import::from_pmx(path, renderer)?;
		
		Ok(MMDModel::new(Arc::new(shared), renderer)?)
	}
	
	pub fn loaded(&self) -> bool {
		self.shared.fences.iter().all(|fence| fence.check())
	}
}

impl<VI: VertexIndex> Model for MMDModel<VI> {
	fn pre_render(&mut self, builder: &mut AutoCommandBufferBuilder<StandardCommandPoolBuilder>, _model_matrix: &AMat4, bones: &[Bone], morphs: &[f32]) -> Result<(), ModelRenderError> {
		for bone in bones {
			let transform = match bone.parent {
				None => &bone.local_transform * &bone.anim_transform.to_transform(),
				Some(id) => &self.bones_mats[id] * &bone.local_transform * &bone.anim_transform,
			};
		
			self.bones_mats.push(transform);
		}
		
		for (id, mat) in self.bones_mats.iter_mut().enumerate() {
			*mat = *mat * &bones[id].inv_model_transform;
		}
		
		let bone_buf = self.shared.bones_pool.chunk(self.bones_mats.drain(..))?;
		builder.copy_buffer(bone_buf, self.bones_ubo.clone())?;
		
		self.morphs_vec.clear();
		let mut max_size = 0;
		let mut packing = false;
		for (id, scale) in morphs.iter().enumerate() {
			if scale.abs() > f32::EPSILON {
				if packing {
					if let Some(last) = self.morphs_vec.last_mut() {
						last.z = id as i32;
						last.w = scale.to_bits() as i32;
					}
				} else {
					self.morphs_vec.push(IVec4::new(id as i32, scale.to_bits() as i32, 0, 0));
				}
				
				packing = !packing;
				
				if self.shared.morphs_sizes[id] > max_size {
					max_size = self.shared.morphs_sizes[id];
				}
			}
		}
		
		if self.morphs_vec.is_empty() {
			builder.fill_buffer(self.offsets_ubo.clone(), 0)?;
		} else {
			let groups = (max_size + GROUP_SIZE - 1) / GROUP_SIZE;
			
			let morph_buf = self.shared.morphs_pool.chunk(self.morphs_vec.iter().copied())?;
			
			builder.copy_buffer(morph_buf, self.morphs_ubo.clone())?
			       .fill_buffer(self.offsets_ubo.clone(), 0)?
			       .dispatch([groups as u32, self.morphs_vec.len() as u32 * 2, 1],
			                 self.shared.morphs_pipeline.clone(),
			                 self.morphs_set.clone(),
			                 ())?;
		}
		
		Ok(())
	}
	
	fn render(&mut self, builder: &mut AutoCommandBufferBuilder, model_matrix: &AMat4, eye: u32) -> Result<(), ModelRenderError> {
		if !self.loaded() { return Ok(()) }
		
		// Outline
		for sub_mesh in self.shared.sub_mesh.iter() {
			if let Some((pipeline, mesh_set)) = sub_mesh.edge.clone() {
				let index_buffer = self.shared.indices.clone().into_buffer_slice().slice(sub_mesh.range.clone()).unwrap();
		
				// calculate size of one pixel at distance 1m from camera
				// Assume index
				// 1440×1600 110 FOV
				let pixel = (110.0_f32 / 360.0 * std::f32::consts::PI).tan() * 2.0 / 1440.0;
				let scale: f32 = pixel * sub_mesh.edge_scale;
				
				builder.draw_indexed(pipeline,
				                     &DynamicState::none(),
				                     self.shared.vertices.clone(),
				                     index_buffer.clone(),
				                     (self.model_set.clone(), mesh_set),
				                     (model_matrix.to_homogeneous(), sub_mesh.edge_color, eye, scale))?;
			}
		}
		
		// Opaque
		for sub_mesh in self.shared.sub_mesh.iter() {
			let index_buffer = self.shared.indices.clone().into_buffer_slice().slice(sub_mesh.range.clone()).unwrap();
			let (pipeline, mesh_set) = sub_mesh.main.clone();
		
			builder.draw_indexed(pipeline,
			                     &DynamicState::none(),
			                     self.shared.vertices.clone(),
			                     index_buffer.clone(),
			                     (self.model_set.clone(), mesh_set),
			                     (model_matrix.to_homogeneous(), eye))?;
		}
		
		// Transparent
		for sub_mesh in self.shared.sub_mesh.iter() {
			if let Some((pipeline, mesh_set)) = sub_mesh.transparent.clone() {
				let index_buffer = self.shared.indices.clone().into_buffer_slice().slice(sub_mesh.range.clone()).unwrap();
		
				builder.draw_indexed(pipeline,
				                     &DynamicState::none(),
				                     self.shared.vertices.clone(),
				                     index_buffer.clone(),
				                     (self.model_set.clone(), mesh_set),
				                     (model_matrix.to_homogeneous(), eye))?;
			}
		}
		
		Ok(())
	}
	
	fn get_default_bones(&self) -> &[Bone] {
		&self.shared.default_bones
	}
	
	fn morphs_count(&self) -> usize {
		self.shared.morphs_sizes.len()
	}
	
	fn try_clone(&self, renderer: &mut Renderer) -> Result<Box<dyn Model>, ModelError> {
		Ok(Box::new(MMDModel::new(self.shared.clone(), renderer)?))
	}
}
