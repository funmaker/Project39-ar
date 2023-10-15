use std::io::Cursor;
use image::{DynamicImage, ImageFormat};
use vulkano::buffer::{Buffer, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract};
use vulkano::format::Format;
use vulkano::image::{ImmutableImage, MipmapsCount, ImageDimensions};

use crate::math::{IVec4, Vec3};
use crate::renderer::Renderer;
use crate::utils::{ImageEx, FenceCheck, BufferEx, IntoInfo};
use super::super::super::{ModelError, VertexIndex};
use super::super::pipeline::{MMDPipelineMorphs, MORPH_GROUP_SIZE};
use super::{MMDModelShared, Vertex, BoneDesc, SubMesh, SubMeshDesc, ColliderDesc, JointDesc, MaterialInfo};


pub struct MMDModelSharedBuilder<VI: VertexIndex> {
	vertices: Vec<Vertex>,
	indices: Vec<VI>,
	textures: Vec<DynamicImage>,
	sub_meshes: Vec<SubMeshDesc>,
	bones: Vec<BoneDesc>,
	morphs: Vec<Vec<(VI, Vec3)>>,
	colliders: Vec<ColliderDesc>,
	joints: Vec<JointDesc>,
}

impl<VI: VertexIndex> MMDModelSharedBuilder<VI> {
	pub fn new(vertices: Vec<Vertex>, indices: Vec<VI>) -> Self {
		MMDModelSharedBuilder {
			vertices,
			indices,
			textures: vec![],
			sub_meshes: vec![],
			bones: vec![],
			morphs: vec![],
			colliders: vec![],
			joints: vec![],
		}
	}
	
	pub fn add_texture(&mut self, texture: DynamicImage) -> &mut Self {
		self.textures.push(texture);
		self
	}
	
	pub fn add_sub_mesh(&mut self, sub_mesh: SubMeshDesc) -> &mut Self {
		self.sub_meshes.push(sub_mesh);
		self
	}
	
	pub fn add_bone(&mut self, bone: BoneDesc) -> &mut Self {
		self.bones.push(bone.id(self.bones.len()));
		self
	}
	
	pub fn add_morph(&mut self, offsets: Vec<(VI, Vec3)>) -> &mut Self {
		self.morphs.push(offsets);
		self
	}
	
	pub fn add_collider(&mut self, desc: ColliderDesc) -> &mut Self {
		self.colliders.push(desc);
		self
	}
	
	pub fn add_joint(&mut self, desc: JointDesc) -> &mut Self {
		self.joints.push(desc);
		self
	}
	
	pub fn build(mut self, renderer: &mut Renderer) -> Result<MMDModelShared, ModelError> {
		let mut upload_buffer = AutoCommandBufferBuilder::primary(&*renderer.command_buffer_allocator,
		                                                          renderer.load_queue.queue_family_index(),
		                                                          CommandBufferUsage::OneTimeSubmit)?;
		
		let vertices = Buffer::upload_iter(&renderer.memory_allocator,
		                                   BufferUsage::VERTEX_BUFFER.into_info(),
		                                   self.vertices.into_iter(),
		                                   &mut upload_buffer)?;
		
		let indices = Buffer::upload_iter(&renderer.memory_allocator,
		                                  BufferUsage::INDEX_BUFFER.into_info(),
		                                  self.indices.into_iter(),
		                                  &mut upload_buffer)?;
		
		let default_tex = {
			let texture_reader = Cursor::new(&include_bytes!("../default_tex.png")[..]);
			let image = image::load(texture_reader, ImageFormat::Png)?;
			let width = image.width();
			let height = image.height();
			
			ImmutableImage::from_iter(&renderer.memory_allocator,
			                          image.into_pre_mul_iter(),
			                          ImageDimensions::Dim2d{ width, height, array_layers: 1 },
			                          MipmapsCount::Log2,
			                          Format::R8G8B8A8_SRGB,
			                          &mut upload_buffer)?
		};
		
		let mut images = vec![];
		
		for texture in self.textures {
			let width = texture.width();
			let height = texture.height();
			
			let image = ImmutableImage::from_iter(&renderer.memory_allocator,
			                                      texture.into_pre_mul_iter(),
			                                      ImageDimensions::Dim2d{ width, height, array_layers: 1 },
			                                      MipmapsCount::Log2,
			                                      Format::R8G8B8A8_SRGB,
			                                      &mut upload_buffer)?;
			
			images.push(image);
		}
		
		let mut sub_meshes = vec![];
		
		for desc in self.sub_meshes {
			let texture = desc.texture.and_then(|id| images.get(id))
			                  .cloned()
			                  .unwrap_or_else(|| default_tex.clone());
			
			let toon = desc.toon.and_then(|id| images.get(id))
			               .cloned()
			               .unwrap_or_else(|| default_tex.clone());
			
			let sphere_map = desc.sphere_map.and_then(|id| images.get(id))
			                     .cloned()
			                     .unwrap_or_else(|| default_tex.clone());
			
			let material_info = MaterialInfo {
				color: desc.color.into(),
				specular: desc.specular.into(),
				specularity: desc.specularity,
				ambient: desc.ambient.into(),
				sphere_mode: desc.sphere_mode,
			};
			
			let material_buffer = Buffer::upload_data(&renderer.memory_allocator,
			                                          BufferUsage::UNIFORM_BUFFER.into_info(),
			                                          material_info,
			                                          &mut upload_buffer)?;
			
			let sub_mesh = SubMesh::new(desc.range, material_buffer, texture, toon, sphere_map, desc.opaque, desc.no_cull, desc.edge, renderer)?;
			
			sub_meshes.push(sub_mesh);
		}
		
		let default_bones = self.bones;
		
		// Create fake null morph if there is no morphs
		if self.morphs.is_empty() {
			self.morphs.push(vec![(VI::zeroed(), Vec3::zeros())])
		}
		
		let morphs_sizes = self.morphs.iter()
		                       .map(|v| v.len())
		                       .collect::<Vec<_>>();
		let morphs_max_size = morphs_sizes.iter().copied().max().unwrap_or(MORPH_GROUP_SIZE);
		let morphs_max_size = (morphs_max_size + MORPH_GROUP_SIZE - 1) / MORPH_GROUP_SIZE * MORPH_GROUP_SIZE;
		
		let morphs_offsets = {
			let mut offsets = vec![IVec4::zeros().into(); morphs_max_size * self.morphs.len()];
			
			for (mid, morph) in self.morphs.into_iter().enumerate() {
				for (oid, (index, offset)) in morph.into_iter().enumerate() {
					offsets[mid * morphs_max_size + oid] = vector!((offset.x * 1_000_000.0) as i32,
					                                               (offset.y * 1_000_000.0) as i32,
					                                               (offset.z * 1_000_000.0) as i32,
					                                               Into::<u32>::into(index) as i32).into();
				}
			}
			
			Buffer::upload_iter(&renderer.memory_allocator,
			                    (BufferUsage::STORAGE_BUFFER | BufferUsage::UNIFORM_BUFFER).into_info(),
			                    offsets.into_iter(),
			                    &mut upload_buffer)?
		};
		
		let morphs_pipeline = renderer.pipelines.get::<MMDPipelineMorphs>()?;
		
		let upload_future = upload_buffer.build()?
		                                 .execute(renderer.load_queue.clone())?;
		
		let fence = FenceCheck::new(upload_future)?;
		
		Ok(MMDModelShared {
			vertices,
			indices: indices.into(),
			sub_meshes,
			default_bones,
			morphs_offsets,
			morphs_sizes,
			morphs_max_size,
			morphs_pipeline,
			fence,
			colliders: self.colliders,
			joints: self.joints,
		})
	}
}
