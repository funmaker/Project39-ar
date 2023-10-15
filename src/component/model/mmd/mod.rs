use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use egui::Ui;
use nalgebra::UnitQuaternion;
use rapier3d::dynamics::{JointAxis, RigidBodyType};
use rapier3d::geometry::Collider;
use rapier3d::prelude::GenericJoint;
use simba::scalar::SubsetOf;
use vulkano::buffer::{Buffer, Subbuffer, BufferUsage};
use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::command_buffer::CopyBufferInfo;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::memory::allocator::MemoryUsage;
use vulkano::pipeline::{Pipeline, PipelineBindPoint};

pub mod asset;
pub mod pipeline;
pub mod shared;
pub mod test;
mod bone;
mod overrides;
mod rigid_body;

use crate::debug;
use crate::application::{Application, Entity};
use crate::math::{AMat4, Color, Isometry3, IVec4, Mat4, PI};
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::utils::{AutoCommandBufferBuilderEx, ExUi, IntoInfo, SubbufferAllocatorEx};
use super::super::{Component, ComponentBase, ComponentRef, ComponentError, ComponentInner};
use super::super::physics::collider::ColliderComponent;
use super::super::physics::joint::JointComponent;
use super::ModelError;
pub use bone::MMDBone;
pub use overrides::BodyPart;
pub use pipeline::{MORPH_GROUP_SIZE, Vertex, Pc};
pub use rigid_body::MMDRigidBody;
use shared::{MMDModelShared, BoneConnection};


pub struct MMDModelState {
	pub bones: Vec<MMDBone>,
	pub rigid_bodies: Vec<ComponentRef<MMDRigidBody>>,
	pub joints: Vec<ComponentRef<JointComponent>>,
	pub morphs: Vec<f32>,
	
	bones_mats: Vec<AMat4>,
	morphs_vec: Vec<IVec4>,
	selected_bone: Option<usize>,
}

#[derive(ComponentBase)]
pub struct MMDModel {
	#[inner] inner: ComponentInner,
	pub state: RefCell<MMDModelState>,
	shared: Arc<MMDModelShared>,
	bones_ubo: Subbuffer<[Mat4]>,
	morphs_ubo: Subbuffer<[IVec4]>,
	offsets_ubo: Subbuffer<[IVec4]>,
	morphs_set: Arc<PersistentDescriptorSet>,
	model_set: Arc<PersistentDescriptorSet>,
	model_edge_set: Option<Arc<PersistentDescriptorSet>>,
	upload_allocator: SubbufferAllocator,
}

#[allow(dead_code)]
impl MMDModel {
	pub fn new(shared: Arc<MMDModelShared>, renderer: &mut Renderer) -> Result<MMDModel, ModelError> {
		let bones = shared.default_bones.iter().map(Into::into).collect::<Vec<_>>();
		let bones_count = bones.len();
		let bones_mats = Vec::with_capacity(bones_count);
		let bones_ubo = Buffer::new_slice(&renderer.memory_allocator,
		                                  (BufferUsage::TRANSFER_DST | BufferUsage::STORAGE_BUFFER).into_info(),
		                                  MemoryUsage::DeviceOnly.into_info(),
		                                  bones_count as u64)?;
		
		let morphs = vec![0.0; shared.morphs_sizes.len()];
		let morphs_vec_count = (shared.morphs_sizes.len() + 1) / 2;
		let morphs_vec = Vec::with_capacity(morphs_vec_count);
		let morphs_ubo = Buffer::new_slice(&renderer.memory_allocator,
		                                   (BufferUsage::TRANSFER_DST | BufferUsage::STORAGE_BUFFER).into_info(),
		                                   MemoryUsage::DeviceOnly.into_info(),
		                                   morphs_vec_count as u64)?;
		
		let offsets_ubo = Buffer::new_slice(&renderer.memory_allocator,
		                                    (BufferUsage::TRANSFER_DST | BufferUsage::STORAGE_BUFFER).into_info(),
		                                    MemoryUsage::DeviceOnly.into_info(),
		                                    shared.vertices.len() as u64)?;
		
		let upload_allocator = SubbufferAllocator::new(renderer.memory_allocator.clone(),
		                                               SubbufferAllocatorCreateInfo {
			                                               buffer_usage: BufferUsage::TRANSFER_SRC,
			                                               memory_usage: MemoryUsage::Upload,
			                                               ..SubbufferAllocatorCreateInfo::default()
		                                               });
		
		let compute_layout = shared.morphs_pipeline
		                           .layout()
		                           .set_layouts()
		                           .get(0)
		                           .ok_or(ModelError::NoLayout)?
		                           .clone();
		
		let morphs_set = PersistentDescriptorSet::new(&renderer.descriptor_set_allocator,
		                                              compute_layout, [
			                                              WriteDescriptorSet::buffer(0, morphs_ubo.clone()),
			                                              WriteDescriptorSet::buffer(1, shared.morphs_offsets.clone()),
			                                              WriteDescriptorSet::buffer(2, offsets_ubo.clone()),
		                                              ])?;
		
		let (main_layout, edge_layout) = shared.layouts()?;
		
		let model_set = PersistentDescriptorSet::new(&renderer.descriptor_set_allocator,
		                                             main_layout, [
			                                             WriteDescriptorSet::buffer(0, renderer.commons.clone()),
			                                             WriteDescriptorSet::buffer(1, bones_ubo.clone()),
			                                             WriteDescriptorSet::buffer(2, offsets_ubo.clone()),
		                                             ])?;
		
		let model_edge_set = edge_layout.map(|edge_layout|
			PersistentDescriptorSet::new(&renderer.descriptor_set_allocator,
			                             edge_layout, [
				                             WriteDescriptorSet::buffer(0, renderer.commons.clone()),
				                             WriteDescriptorSet::buffer(1, bones_ubo.clone()),
				                             WriteDescriptorSet::buffer(2, offsets_ubo.clone()),
			                             ])
		).transpose()?;
		
		Ok(MMDModel {
			inner: ComponentInner::from_render_type(RenderType::Transparent),
			state: RefCell::new(MMDModelState {
				bones,
				rigid_bodies: vec![],
				joints: vec![],
				morphs,
				bones_mats,
				morphs_vec,
				selected_bone: None,
			}),
			shared,
			bones_ubo,
			morphs_ubo,
			morphs_set,
			offsets_ubo,
			model_set,
			model_edge_set,
			upload_allocator,
		})
	}
	
	pub fn loaded(&self) -> bool {
		self.shared.fence.check()
	}
	
	fn draw_debug_bones(&self, model_matrix: Isometry3, bones: &[MMDBone], bones_mats: &[AMat4], selected: Option<usize>) {
		for (id, bone) in bones.iter().enumerate() {
			if bone.display {
				let pos = model_matrix.transform_point(&bones_mats[id].transform_point(&bone.origin()));
				
				let color = if selected.is_none() || Some(id) == selected {
					bone.color
				} else {
					Color::BLACK.opactiy(0.5)
				};
				
				debug::draw_point(pos, 10.0, color);
				debug::draw_text(&bone.name, pos, debug::DebugOffset::bottom_right(8.0, 8.0), 32.0, color);
				
				match &bone.connection {
					BoneConnection::None => {}
					BoneConnection::Bone(con) => {
						let cpos = model_matrix.transform_point(&bones_mats[*con].transform_point(&bones[*con].origin()));
						debug::draw_line(pos, cpos, 3.0, color);
					}
					BoneConnection::Offset(cpos) => {
						let cpos = model_matrix.transform_point(&bones_mats[id].transform_point(&(&bone.origin() + cpos)));
						debug::draw_line(pos, cpos, 3.0, color);
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
		let state = &mut *self.state.borrow_mut();
		let ent_pos = *entity.state().position;
		
		for bone in &mut state.bones {
			bone.model = self.as_cref();
		}
		
		let mut rigid_bodies = BTreeMap::new();
		
		for desc in self.shared.joints.iter() {
			let parent_bone_id = self.shared.colliders[desc.collider_a].bone.min(self.shared.colliders[desc.collider_b].bone);
			let bone_id = self.shared.colliders[desc.collider_a].bone.max(self.shared.colliders[desc.collider_b].bone);
			
			// Check if joint divides the bone tree
			if state.bone_ancestors_iter(bone_id)
			        .all(|id| parent_bone_id != id) {
				continue;
			}
			
			let rb_ent = Entity::builder(&desc.name)
				.position(ent_pos * desc.position)
				.gravity_scale(0.05)
				.rigid_body_type(RigidBodyType::Dynamic)
				.build();
			
			rigid_bodies.insert(bone_id, rb_ent);
		}
		
		for desc in self.shared.colliders.iter() {
			let rb = state.bone_ancestors_iter(desc.bone)
			              .find(|bone| rigid_bodies.contains_key(bone))
			              .and_then(|bone| rigid_bodies.get(&bone))
			              .unwrap_or(entity);
			
			let mut collider: Collider = desc.collider.clone();
			collider.set_position(
				rb.state().position.inverse() *
				ent_pos *
				collider.position()
			);
			
			rb.add_component(ColliderComponent::new(collider));
		}
		
		state.rigid_bodies.push(entity.add_component(MMDRigidBody::new(0, Some(BodyPart::Hip), ComponentRef::null())));
		
		for desc in self.shared.joints.iter() {
			let (bone_a, rb_a) = state.bone_ancestors_iter(self.shared.colliders[desc.collider_a].bone)
			                          .find(|bone| rigid_bodies.contains_key(bone))
			                          .and_then(|bone| rigid_bodies.get(&bone).map(|rb| (bone, rb)))
			                          .unwrap_or((0, entity));
			let (bone_b, rb_b) = state.bone_ancestors_iter(self.shared.colliders[desc.collider_b].bone)
			                          .find(|bone| rigid_bodies.contains_key(bone))
			                          .and_then(|bone| rigid_bodies.get(&bone).map(|rb| (bone, rb)))
			                          .unwrap_or((0, entity));
			
			let mut joint = GenericJoint::default();
			
			joint.set_local_frame1(rb_a.state().position.inverse() * ent_pos * desc.position)
			     .set_local_frame2(rb_b.state().position.inverse() * ent_pos * desc.position);
			
			fn limit(mut joint: GenericJoint, axis: JointAxis, min: f32, max: f32, max_limit: f32) -> GenericJoint {
				if max - min >= max_limit || min > max {
					// free
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
			
			joint = limit(joint, JointAxis::AngX,  desc.rotation_min.x,  desc.rotation_max.x, PI * 2.0);
			joint = limit(joint, JointAxis::AngY, -desc.rotation_max.y, -desc.rotation_min.y, PI * 2.0);
			joint = limit(joint, JointAxis::AngZ, -desc.rotation_max.z, -desc.rotation_min.z, PI * 2.0);
			
			let joint_ref = rb_a.add_component(JointComponent::new(joint, rb_b.as_ref()).named(&desc.name));
			
			state.joints.push(joint_ref.clone());
			
			let parent_bone_id = bone_a.min(bone_b);
			let bone_id = bone_a.max(bone_b);
			
			if state.bone_ancestors_iter(bone_id)
			        .any(|id| parent_bone_id == id) {
				if bone_a < bone_b {
					rb_b.set_parent(rb_a.as_ref(), false, application);
					state.rigid_bodies.push(rb_b.add_component(MMDRigidBody::new(bone_b, desc.body_part, joint_ref)));
				} else {
					rb_a.set_parent(rb_b.as_ref(), false, application);
					state.rigid_bodies.push(rb_a.add_component(MMDRigidBody::new(bone_a, desc.body_part, joint_ref)));
				}
			}
		}
		
		state.bones[0].attach_rigid_body(entity.as_ref(), Isometry3::identity());
		
		for (bone_id, rb) in rigid_bodies {
			let offset = entity.state().position.inverse() * *rb.state().position;
			state.bones[bone_id].attach_rigid_body(application.add_entity(rb), offset);
		}
		
		Ok(())
	}
	
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		let state = &mut *self.state.borrow_mut();
		let inv_ent_pos = entity.state().position.inverse();
		
		for bone in state.bones.iter_mut() {
			if let Some(rb) = bone.rigid_body.get(application) {
				bone.transform_override = Some((
					inv_ent_pos *
					*rb.state().position *
					bone.inv_rigid_body_transform
				).to_superset());
			}
		}
		
		if application.get_selection().mmd_model() == self.as_cref() {
			state.selected_bone = application.get_selection().mmd_bone();
		} else {
			state.selected_bone = None;
		}
		
		Ok(())
	}
	
	fn before_render(&self, entity: &Entity, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<(), ComponentError> {
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
			self.draw_debug_bones(*entity.state().position, &state.bones, &state.bones_mats, state.selected_bone);
		}
		
		let bone_buf = self.upload_allocator.from_iter(state.bones_mats.drain(..).map(|mat| {
			let mut mat = mat.to_homogeneous();
			let rot = UnitQuaternion::from_matrix(&mat.fixed_resize(0.0)).inverse();
			mat.set_row(3, &rot.coords.transpose());
			mat
		}))?;
		
		context.builder.copy_buffer(CopyBufferInfo::buffers(bone_buf, self.bones_ubo.clone()))?;
		
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
			context.builder.fill_buffer(self.offsets_ubo.as_bytes().clone().cast_aligned(), 0)?;
		} else {
			let groups = (max_size + MORPH_GROUP_SIZE - 1) / MORPH_GROUP_SIZE;
			
			let morph_buf = self.upload_allocator.from_iter(state.morphs_vec.iter().copied())?;
			
			context.builder.copy_buffer(CopyBufferInfo::buffers(morph_buf, self.morphs_ubo.clone()))?
			               .fill_buffer(self.offsets_ubo.as_bytes().clone().cast_aligned(), 0)?
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
	
	fn render(&self, entity: &Entity, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<(), ComponentError> {
		if !self.loaded() { return Ok(()) }
		let model_matrix = entity.state().position.to_homogeneous();
		
		context.builder.bind_vertex_buffers(0, self.shared.vertices.clone())
		               .bind_any_index_buffer(self.shared.indices.clone());
		
		// Opaque
		for sub_mesh in self.shared.sub_meshes.iter() {
			let (pipeline, mesh_set) = sub_mesh.main.clone();
			
			context.builder.bind_pipeline_graphics(pipeline.clone())
			               .bind_descriptor_sets(PipelineBindPoint::Graphics,
			                                     pipeline.layout().clone(),
			                                     0,
			                                     (self.model_set.clone(), mesh_set))
			               .push_constants(self.shared.sub_meshes.first().unwrap().main.0.layout().clone(),
			                               0,
			                               Pc {
				                               model: model_matrix.into(),
				                               color: Color::WHITE.into(),
				                               scale: 1.0,
			                               })
			               .draw_indexed(sub_mesh.range.len() as u32,
			                             1,
			                             sub_mesh.range.start,
			                             0,
			                             0)?;
		}
		
		// Transparent
		for sub_mesh in self.shared.sub_meshes.iter() {
			if let Some((pipeline, mesh_set)) = sub_mesh.transparent.clone() {
				context.builder.bind_pipeline_graphics(pipeline.clone())
				               .bind_descriptor_sets(PipelineBindPoint::Graphics,
				                                     pipeline.layout().clone(),
				                                     0,
				                                     (self.model_set.clone(), mesh_set))
				               .push_constants(self.shared.sub_meshes.first().unwrap().main.0.layout().clone(),
				                               0,
				                               Pc {
					                               model: model_matrix.into(),
					                               color: Color::WHITE.into(),
					                               scale: 1.0,
				                               })
				               .draw_indexed(sub_mesh.range.len() as u32,
				                             1,
				                             sub_mesh.range.start,
				                             0,
				                             0)?;
			}
		}
		
		// Outline
		for sub_mesh in self.shared.sub_meshes.iter() {
			if let Some((pipeline, mesh_set)) = sub_mesh.edge.clone() {
				let edge_scale = (context.fov.0.x / 2.0).tan() * 2.0 * context.pixel_scale.x * sub_mesh.edge_scale;
		
				context.builder.bind_pipeline_graphics(pipeline.clone())
				       .bind_descriptor_sets(PipelineBindPoint::Graphics,
				                             pipeline.layout().clone(),
				                             0,
				                             (self.model_edge_set.clone().unwrap(), mesh_set))
				       .push_constants(pipeline.layout().clone(),
				                       0,
				                       Pc {
					                       model: model_matrix.into(),
					                       color: sub_mesh.edge_color.into(),
					                       scale: edge_scale,
				                       })
				       .draw_indexed(sub_mesh.range.len() as u32,
				                     1,
				                     sub_mesh.range.start,
				                     0,
				                     0)?;
			}
		}
		
		Ok(())
	}
	
	fn on_inspect_extra(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		use egui::*;
		
		if let Ok(mut state) = self.state.try_borrow_mut() {
			CollapsingHeader::new(format!("Bones ({})", state.bones.len()))
				.id_source("Bones")
				.show(ui, |ui| {
					for bone in &mut state.bones {
						ui.inspect_collapsing()
						  .show(ui, bone, application)
					}
				});
			
			CollapsingHeader::new(format!("Rigid Bodies ({})", state.rigid_bodies.len()))
				.id_source("Rigid Bodies")
				.show(ui, |ui| {
					for rb in &state.rigid_bodies {
						ui.inspect_collapsing()
						  .maybe_title(rb.entity().get(application).map(|rb| &rb.name))
						  .show(ui, rb, application)
					}
				});
			
			CollapsingHeader::new(format!("Joints ({})", state.joints.len()))
				.id_source("Joints")
				.show(ui, |ui| {
					for joint in &state.joints {
						ui.inspect_collapsing()
						  .maybe_title(joint.get(application).map(|joint| &joint.name))
						  .show(ui, joint, application)
					}
				});
			
			CollapsingHeader::new(format!("Morphs ({})", state.morphs.len()))
				.id_source("Morphs")
				.show(ui, |ui| {
					Grid::new("Morphs")
						.min_col_width(100.0)
						.num_columns(2)
						.show(ui, |ui| {
							for (id, morph) in state.morphs.iter_mut().enumerate() {
								ui.inspect_row(format!("{}", id), morph, (0.1, 0.0..=1.0))
							}
						});
				});
		} else {
			ui.label("Can't borrow state.");
		}
	}
}

impl MMDModelState {
	fn bone_ancestors_iter(&self, mut bone_id: usize) -> impl Iterator<Item = usize> + '_ {
		Some(bone_id).into_iter()
		             .chain(std::iter::from_fn(move || {
			             if let Some(parent_id) = self.bones[bone_id].parent {
				             bone_id = parent_id;
				             Some(parent_id)
			             } else {
				             None
			             }
		             }))
		             .chain(Some(0))
	}
	
	// fn find_rb(&self, mut bone_id: usize) -> &MMDRigidBody {
	// 	&self.rigid_bodies[self.find_rb_index(bone_id)]
	// }
}
