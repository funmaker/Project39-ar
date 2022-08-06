use std::cell::RefCell;
use std::mem::size_of;
use std::sync::Arc;
use std::time::Duration;
use nalgebra::UnitQuaternion;
use num_traits::Zero;
use rapier3d::dynamics::{JointAxis, RigidBodyBuilder, RigidBodyType};
use rapier3d::geometry::Collider;
use rapier3d::prelude::GenericJoint;
use simba::scalar::SubsetOf;
use vulkano::buffer::{BufferUsage, DeviceLocalBuffer, TypedBufferAccess};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::DeviceOwned;
use vulkano::DeviceSize;
use vulkano::pipeline::{Pipeline, PipelineBindPoint};

pub mod shared;
pub mod asset;
pub mod test;
mod bone;
mod rigid_body;
mod config;

pub use crate::renderer::pipelines::mmd::{MORPH_GROUP_SIZE, Vertex};
use crate::renderer::Renderer;
use crate::application::{Application, Entity};
use crate::utils::{AutoCommandBufferBuilderEx, get_userdata, NgPod};
use crate::component::{Component, ComponentBase, ComponentError, ComponentInner, RenderType};
use crate::debug;
use crate::math::{AMat4, Isometry3, IVec4, Vec4, Vec3, PI, Mat4};
use super::ModelError;
pub use bone::{MMDBone, BoneConnection};
pub use rigid_body::MMDRigidBody;
use shared::MMDModelShared;

pub struct MMDModelState {
	pub bones: Vec<MMDBone>,
	pub rigid_bodies: Vec<MMDRigidBody>,
	pub morphs: Vec<f32>,
	
	bones_mats: Vec<AMat4>,
	morphs_vec: Vec<IVec4>,
}

#[derive(ComponentBase)]
pub struct MMDModel {
	#[inner] inner: ComponentInner,
	pub state: RefCell<MMDModelState>,
	shared: Arc<MMDModelShared>,
	bones_ubo: Arc<DeviceLocalBuffer<[NgPod<Mat4>]>>,
	morphs_ubo: Arc<DeviceLocalBuffer<[NgPod<IVec4>]>>,
	offsets_ubo: Arc<DeviceLocalBuffer<[NgPod<IVec4>]>>,
	morphs_set: Arc<PersistentDescriptorSet>,
	model_set: Arc<PersistentDescriptorSet>,
}

#[allow(dead_code)]
impl MMDModel {
	pub fn new(shared: Arc<MMDModelShared>, renderer: &mut Renderer) -> Result<MMDModel, ModelError> {
		let bones = shared.default_bones.clone();
		let bones_count = bones.len();
		let bones_mats = Vec::with_capacity(bones_count);
		let bones_ubo = DeviceLocalBuffer::array(shared.vertices.device().clone(),
		                                         (size_of::<AMat4>() * bones_count) as DeviceSize,
		                                         BufferUsage {
			                                         transfer_destination: true,
			                                         storage_buffer: true,
			                                         ..BufferUsage::none()
		                                         },
		                                         Some(renderer.queue.family()))?;
		
		let morphs = vec![0.0; shared.morphs_sizes.len()];
		let morphs_vec_count = (shared.morphs_sizes.len() + 1) / 2;
		let morphs_vec = Vec::with_capacity(morphs_vec_count);
		let morphs_ubo = DeviceLocalBuffer::array(shared.vertices.device().clone(),
		                                          morphs_vec_count as DeviceSize,
		                                          BufferUsage {
			                                          transfer_destination: true,
			                                          storage_buffer: true,
			                                          ..BufferUsage::none()
		                                          },
		                                          Some(renderer.queue.family()))?;
		
		let offsets_ubo = DeviceLocalBuffer::array(shared.vertices.device().clone(),
		                                           shared.vertices.len(),
		                                           BufferUsage {
			                                           transfer_destination: true,
			                                           storage_buffer: true,
			                                           ..BufferUsage::none()
		                                           },
		                                           Some(renderer.queue.family()))?;
		
		let compute_layout = shared.morphs_pipeline
		                           .layout()
		                           .set_layouts()
		                           .get(0)
		                           .ok_or(ModelError::NoLayout)?
		                           .clone();
		
		let morphs_set = PersistentDescriptorSet::new(compute_layout, [
			WriteDescriptorSet::buffer(0, morphs_ubo.clone()),
			WriteDescriptorSet::buffer(1, shared.morphs_offsets.clone()),
			WriteDescriptorSet::buffer(2, offsets_ubo.clone()),
		])?;
		
		let model_set = PersistentDescriptorSet::new(shared.commons_layout(renderer)?, [
			WriteDescriptorSet::buffer(0, renderer.commons.clone()),
			WriteDescriptorSet::buffer(1, bones_ubo.clone()),
			WriteDescriptorSet::buffer(2, offsets_ubo.clone()),
		])?;
		
		Ok(MMDModel {
			inner: ComponentInner::from_render_type(RenderType::Opaque),
			state: RefCell::new(MMDModelState {
				bones,
				morphs,
				bones_mats,
				rigid_bodies: vec![],
				morphs_vec,
			}),
			shared,
			bones_ubo,
			morphs_ubo,
			morphs_set,
			offsets_ubo,
			model_set,
		})
	}
	
	pub fn loaded(&self) -> bool {
		self.shared.fence.check()
	}
	
	fn draw_debug_bones(&self, model_matrix: Isometry3, bones: &[MMDBone], bones_mats: &[AMat4]) {
		for (id, bone) in bones.iter().enumerate() {
			if bone.display {
				let pos = model_matrix.transform_point(&bones_mats[id].transform_point(&bone.origin()));
				
				debug::draw_point(&pos, 10.0, bone.color.clone());
				debug::draw_text(&bone.name, &pos, debug::DebugOffset::bottom_right(8.0, 8.0), 32.0, bone.color.clone());
				
				match &bone.connection {
					BoneConnection::None => {}
					BoneConnection::Bone(con) => {
						let cpos = model_matrix.transform_point(&bones_mats[*con].transform_point(&bones[*con].origin()));
						debug::draw_line(pos, cpos, 3.0, bone.color.clone());
					}
					BoneConnection::Offset(cpos) => {
						let cpos = model_matrix.transform_point(&bones_mats[id].transform_point(&(&bone.origin() + cpos)));
						debug::draw_line(pos, cpos, 3.0, bone.color.clone());
					}
				}
			}
		}
	}
	
	// pub fn try_clone(&self, renderer: &mut Renderer) -> Result<Box<dyn Model>, ModelError> {
	// 	Ok(Box::new(MMDModel::new(self.shared.clone(), renderer)?))
	// }
}

impl Component for MMDModel {
	fn start(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		let physics = &mut *application.physics.borrow_mut();
		let ent_state = entity.state();
		let state = &mut *self.state.borrow_mut();
		
		let root_pos = Isometry3::new(state.bones[0].origin().coords, Vec3::zeros());
		state.rigid_bodies.push(MMDRigidBody::new("Root",
		                                          entity.rigid_body,
		                                          0,
		                                          root_pos,
		                                          root_pos.inverse()));
		
		for desc in self.shared.joints.iter() {
			let parent_bone_id = self.shared.colliders[desc.collider_a].bone.min(self.shared.colliders[desc.collider_b].bone);
			let bone_id = self.shared.colliders[desc.collider_a].bone.max(self.shared.colliders[desc.collider_b].bone);
			
			// Check if joint divides the bone tree
			if state.bone_ancestors_iter(bone_id)
			        .all(|id| parent_bone_id != id) {
				continue;
			}
			
			let position = *ent_state.position * desc.position;
			let inv_bone_pos = desc.position.inverse() * state.bones[bone_id].inv_model_transform.inverse();
			
			let rb = RigidBodyBuilder::new(RigidBodyType::Dynamic)
			                          .position(position)
			                          .gravity_scale(0.05)
			                          .sleeping(false)
			                          .can_sleep(false)
			                          .user_data(get_userdata(entity.id, self.id()))
			                          .build();
			
			let handle = physics.rigid_body_set.insert(rb);
			
			state.rigid_bodies.push(MMDRigidBody::new(&desc.name, handle, bone_id, desc.position, inv_bone_pos))
		}
		
		state.rigid_bodies.sort_by(|a, b| a.bone.cmp(&b.bone));
		
		for desc in self.shared.colliders.iter() {
			let rb_id = state.find_rb_index(desc.bone);
			let rigid_body = &mut state.rigid_bodies[rb_id];
			
			let mut collider: Collider = desc.collider.clone();
			collider.set_position(
				rigid_body.rest_pos.inverse() *
				collider.position()
			);
			collider.user_data = get_userdata(entity.id, self.id());
			
			let handle = physics.collider_set.insert_with_parent(collider, rigid_body.handle, &mut physics.rigid_body_set);
			rigid_body.colliders.push(handle);
		}
		
		for desc in self.shared.joints.iter() {
			let bone_a = self.shared.colliders[desc.collider_a].bone;
			let bone_b = self.shared.colliders[desc.collider_b].bone;
			let rb_a_id = state.find_rb_index(bone_a);
			let rb_b_id = state.find_rb_index(bone_b);
			
			// {
			// 	let parent_bone_id = self.shared.colliders[desc.collider_a].bone.min(self.shared.colliders[desc.collider_b].bone);
			// 	let bone_id = self.shared.colliders[desc.collider_a].bone.max(self.shared.colliders[desc.collider_b].bone);
			//
			// 	// Check if joint divides the bone tree
			// 	if state.bone_ancestors_iter(bone_id)
			// 	        .all(|id| parent_bone_id != id) {
			// 		continue;
			// 	}
			// }
			
			let handle = {
				let rb_a = &state.rigid_bodies[rb_a_id];
				let rb_b = &state.rigid_bodies[rb_b_id];
				
				let mut joint = GenericJoint::default();
				
				joint.set_local_frame1(rb_a.rest_pos.inverse() * desc.position)
				     .set_local_frame2(rb_b.rest_pos.inverse() * desc.position);
				
				fn limit(mut joint: GenericJoint, axis: JointAxis, min: f32, max: f32, max_limit: f32) -> GenericJoint {
					if max - min >= max_limit {
					
					} else if min != max {
						joint.set_limits(axis, [min, max]);
					} else {
						joint.lock_axes(axis.into());
					}
					
					joint
				}
				
				joint = limit(joint, JointAxis::X, desc.position_min.x, desc.position_max.x, 100.0);
				joint = limit(joint, JointAxis::Y, desc.position_min.y, desc.position_max.y, 100.0);
				joint = limit(joint, JointAxis::Z, desc.position_min.z, desc.position_max.z, 100.0);
				
				joint = limit(joint, JointAxis::AngX, desc.rotation_min.x, desc.rotation_max.x, PI * 2.0);
				joint = limit(joint, JointAxis::AngY, desc.rotation_min.y, desc.rotation_max.y, PI * 2.0);
				joint = limit(joint, JointAxis::AngZ, desc.rotation_min.z, desc.rotation_max.z, PI * 2.0);
				
				physics.impulse_joint_set.insert(rb_a.handle,
				                                 rb_b.handle,
				                                 joint,
				                                 true)
			};
			
			if bone_a > bone_b {
				state.rigid_bodies[rb_a_id].joint = handle;
			} else {
				state.rigid_bodies[rb_b_id].joint = handle;
			}
		}
		
		Ok(())
	}
	
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		let physics = &mut *application.physics.borrow_mut();
		let state = &mut *self.state.borrow_mut();
		let inv_ent_pos = entity.state().position.inverse();
		
		for rb in state.rigid_bodies.iter() {
			let rrb = physics.rigid_body_set.get_mut(rb.handle).unwrap();
			state.bones[rb.bone].transform_override = Some((
				inv_ent_pos *
				rrb.position() *
				rb.inv_bone_pos
			).to_superset());
			rrb.wake_up(true);
		}
		
		Ok(())
	}
	
	fn pre_render(&self, entity: &Entity, _renderer: &Renderer, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		let state = &mut *self.state.borrow_mut();
		
		for bone in &state.bones {
			let transform = if let Some(transform) = bone.transform_override {
				transform.to_superset()
			} else if let Some(id) = bone.parent {
				&state.bones_mats[id] * &bone.local_transform * &bone.anim_transform
			} else {
				let transform: AMat4 = bone.anim_transform.to_superset();
				&bone.local_transform * &transform
			};
		
			state.bones_mats.push(transform);
		}
		
		for (id, mat) in state.bones_mats.iter_mut().enumerate() {
			*mat = *mat * &state.bones[id].inv_model_transform;
		}
		
		if debug::get_flag_or_default("DebugBonesDraw") {
			self.draw_debug_bones(*entity.state().position, &state.bones, &state.bones_mats);
		}
		
		let bone_buf = self.shared.bones_pool.chunk(state.bones_mats.drain(..).map(|mat| {
			let mut mat = mat.to_homogeneous();
			let rot = UnitQuaternion::from_matrix(&mat.fixed_resize(0.0)).inverse();
			mat.set_row(3, &rot.coords.transpose());
			mat.into()
		}))?;
		
		builder.copy_buffer(bone_buf, self.bones_ubo.clone())?;
		
		state.morphs_vec.clear();
		let mut max_size = 0;
		let mut packing = false;
		for (id, scale) in state.morphs.iter().enumerate() {
			if scale.abs() > f32::EPSILON {
				if packing {
					if let Some(last) = state.morphs_vec.last_mut() {
						last.z = id as i32;
						last.w = scale.to_bits() as i32;
					}
				} else {
					state.morphs_vec.push(vector!(id as i32, scale.to_bits() as i32, 0, 0));
				}
				
				packing = !packing;
				
				if self.shared.morphs_sizes[id] > max_size {
					max_size = self.shared.morphs_sizes[id];
				}
			}
		}
		
		if state.morphs_vec.is_empty() {
			builder.fill_buffer(self.offsets_ubo.clone(), 0)?;
		} else {
			let groups = (max_size + MORPH_GROUP_SIZE - 1) / MORPH_GROUP_SIZE;
			
			let morph_buf = self.shared.morphs_pool.chunk(state.morphs_vec.iter().copied().map(Into::into))?;
			
			builder.copy_buffer(morph_buf, self.morphs_ubo.clone())?
			       .fill_buffer(self.offsets_ubo.clone(), 0)?
			       .bind_pipeline_compute(self.shared.morphs_pipeline.clone())
			       .bind_descriptor_sets(PipelineBindPoint::Compute,
			                             self.shared.morphs_pipeline.layout().clone(),
			                             0,
			                             self.morphs_set.clone())
			       .push_constants(self.shared.morphs_pipeline.layout().clone(),
			                       0,
			                       self.shared.morphs_max_size as u32)
			       .dispatch([groups as u32, state.morphs_vec.len() as u32 * 2, 1])?;
		}
		
		Ok(())
	}
	
	fn render(&self, entity: &Entity, _renderer: &Renderer, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		if !self.loaded() { return Ok(()) }
		let model_matrix = entity.state().position.to_homogeneous();
		
		builder.bind_vertex_buffers(0, self.shared.vertices.clone())
		       .bind_any_index_buffer(self.shared.indices.clone());
		
		// Outline
		for sub_mesh in self.shared.sub_meshes.iter() {
			if let Some((pipeline, mesh_set)) = sub_mesh.edge.clone() {
				// TODO: Generalize
				// calculate size of one pixel at distance 1m from background
				// Assume index
				// 1440×1600 110 FOV
				let pixel = (110.0_f32 / 360.0 * PI).tan() * 2.0 / 1440.0;
				let scale: f32 = pixel * sub_mesh.edge_scale;
				
				builder.bind_pipeline_graphics(pipeline.clone())
				       .bind_descriptor_sets(PipelineBindPoint::Graphics,
				                             pipeline.layout().clone(),
				                             0,
				                             (self.model_set.clone(), mesh_set))
				       .push_constants(pipeline.layout().clone(),
				                       0,
				                       (model_matrix.clone(), sub_mesh.edge_color, scale))
				       .draw_indexed(sub_mesh.range.len() as u32,
				                     1,
				                     sub_mesh.range.start,
				                     0,
				                     0)?;
			}
		}
		
		// Opaque
		for sub_mesh in self.shared.sub_meshes.iter() {
			let (pipeline, mesh_set) = sub_mesh.main.clone();
			
			builder.bind_pipeline_graphics(pipeline.clone())
			       .bind_descriptor_sets(PipelineBindPoint::Graphics,
			                             pipeline.layout().clone(),
			                             0,
			                             (self.model_set.clone(), mesh_set))
			       .push_constants(self.shared.sub_meshes.first().unwrap().main.0.layout().clone(),
			                       0,
			                       (model_matrix.clone(), Vec4::zero(), 0.0_f32))
			       .draw_indexed(sub_mesh.range.len() as u32,
			                     1,
			                     sub_mesh.range.start,
			                     0,
			                     0)?;
		}
		
		// Transparent
		for sub_mesh in self.shared.sub_meshes.iter() {
			if let Some((pipeline, mesh_set)) = sub_mesh.transparent.clone() {
				builder.bind_pipeline_graphics(pipeline.clone())
				       .bind_descriptor_sets(PipelineBindPoint::Graphics,
				                             pipeline.layout().clone(),
				                             0,
				                             (self.model_set.clone(), mesh_set))
				       .push_constants(self.shared.sub_meshes.first().unwrap().main.0.layout().clone(),
				                       0,
				                       (model_matrix.clone(), Vec4::zero(), 0.0_f32))
				       .draw_indexed(sub_mesh.range.len() as u32,
				                     1,
				                     sub_mesh.range.start,
				                     0,
				                     0)?;
			}
		}
		
		Ok(())
	}
}

impl MMDModelState {
	fn bone_ancestors_iter(&self, mut bone_id: usize) -> impl Iterator<Item = usize> + '_ {
		std::iter::from_fn(move || {
			if let Some(parent_id) = self.bones[bone_id].parent {
				bone_id = parent_id;
				Some(parent_id)
			} else {
				None
			}
		})
	}
	
	fn find_rb_index(&self, mut bone_id: usize) -> usize {
		loop {
			if let Some((index, _)) = self.rigid_bodies.iter()
			                                           .enumerate()
			                                           .find(|(_, rb)| rb.bone == bone_id) {
				return index;
			} else if let Some(parent_id) = self.bones[bone_id].parent {
				bone_id = parent_id;
			} else {
				return 0;
			}
		}
	}
	
	// fn find_rb(&self, mut bone_id: usize) -> &MMDRigidBody {
	// 	&self.rigid_bodies[self.find_rb_index(bone_id)]
	// }
}
