use std::sync::Arc;
use vulkano::command_buffer::pool::standard::StandardCommandPoolBuilder;
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState};
use vulkano::buffer::{BufferUsage, BufferAccess, DeviceLocalBuffer};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::descriptor::DescriptorSet;
use vulkano::device::DeviceOwned;

pub mod test;
mod sub_mesh;
mod import;
mod shared;

use super::{Model, ModelError, ModelRenderError, VertexIndex};
use crate::application::entity::{Bone};
use crate::renderer::Renderer;
use crate::math::{AMat4, ToTransform};
pub use crate::renderer::pipelines::mmd::Vertex;
pub use import::MMDModelLoadError;
use shared::MMDModelShared;

pub struct MMDModel<VI: VertexIndex> {
	shared: Arc<MMDModelShared<VI>>,
	bones_mats: Vec<AMat4>,
	bones_ubo: Arc<DeviceLocalBuffer<[AMat4]>>,
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
		
		let model_set = Arc::new(
			PersistentDescriptorSet::start(shared.commons_layout(renderer)?)
				.add_buffer(renderer.commons.clone())?
				.add_buffer(bones_ubo.clone())?
				.build()?
		);
		
		Ok(MMDModel {
			bones_mats,
			bones_ubo,
			shared,
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
	fn pre_render(&mut self, builder: &mut AutoCommandBufferBuilder<StandardCommandPoolBuilder>, _model_matrix: &AMat4, bones: &Vec<Bone>) -> Result<(), ModelRenderError> {
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
		
		let buffer = self.shared.bones_pool.chunk(self.bones_mats.drain(..))?;
		
		builder.copy_buffer(buffer, self.bones_ubo.clone())?;
		
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
	
	fn try_clone(&self, renderer: &mut Renderer) -> Result<Box<dyn Model>, ModelError> {
		Ok(Box::new(MMDModel::new(self.shared.clone(), renderer)?))
	}
}
