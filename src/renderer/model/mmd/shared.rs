use std::sync::Arc;
use std::ops::Range;
use std::io::Cursor;
use image::{DynamicImage, GenericImageView, ImageFormat};
use vulkano::buffer::{ImmutableBuffer, BufferUsage, CpuBufferPool, BufferAccess, TypedBufferAccess};
use vulkano::image::{ImmutableImage, Dimensions, MipmapsCount};
use vulkano::sync::GpuFuture;
use vulkano::format::Format;
use vulkano::descriptor::descriptor_set::UnsafeDescriptorSetLayout;

use crate::application::entity::Bone;
use crate::renderer::model::{ModelError, VertexIndex, FenceCheck};
use crate::renderer::pipelines::mmd::{MMDPipelineOpaque, MMDPipelineMorphs, MORPH_GROUP_SIZE};
use crate::renderer::Renderer;
use crate::utils::ImageEx;
use crate::math::{AMat4, IVec4, Vec3};
use super::sub_mesh::{SubMesh, MaterialInfo};
use super::Vertex;
use vulkano::descriptor::PipelineLayoutAbstract;

pub struct MMDModelShared<VI: VertexIndex> {
	pub vertices: Arc<ImmutableBuffer<[Vertex]>>,
	pub sub_meshes: Vec<SubMesh<VI>>,
	pub default_bones: Vec<Bone>,
	pub bones_pool: CpuBufferPool<AMat4>,
	pub morphs_offsets: Arc<ImmutableBuffer<[IVec4]>>,
	pub morphs_sizes: Vec<usize>,
	pub morphs_pool: CpuBufferPool<IVec4>,
	pub morphs_pipeline: Arc<MMDPipelineMorphs>,
	pub fence: FenceCheck,
}

impl<VI: VertexIndex> MMDModelShared<VI> {
	pub fn new(vertices: Vec<Vertex>, indices: Vec<VI>) -> MMDModelSharedBuilder<VI> {
		MMDModelSharedBuilder::new(vertices, indices)
	}
	
	pub fn commons_layout(&self, renderer: &mut Renderer) -> Result<Arc<UnsafeDescriptorSetLayout>, ModelError> {
		self.sub_meshes.first()
		               .map(|mesh| mesh.main.0.clone())
		               .ok_or(ModelError::NoLayout)
		               .or_else(|_| renderer.pipelines.get::<MMDPipelineOpaque>().map_err(Into::into).map(Into::into))
		               .and_then(|pipeline| pipeline.descriptor_set_layout(0).cloned().ok_or(ModelError::NoLayout))
	}
}

pub struct MMDModelSharedBuilder<VI: VertexIndex> {
	vertices: Vec<Vertex>,
	indices: Vec<VI>,
	textures: Vec<DynamicImage>,
	sub_meshes: Vec<SubMeshDesc>,
	bones: Vec<Bone>,
	morphs: Vec<Vec<(VI, Vec3)>>,
}

impl<VI: VertexIndex> MMDModelSharedBuilder<VI> {
	pub fn new(vertices: Vec<Vertex>, indices: Vec<VI>) -> Self {
		MMDModelSharedBuilder {
			vertices,
			indices,
			textures: vec![],
			sub_meshes: vec![],
			bones: vec![],
			morphs: vec![]
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
	
	pub fn add_bone(&mut self, bone: Bone) -> &mut Self {
		self.bones.push(bone);
		self
	}
	
	pub fn add_morph(&mut self, offsets: Vec<(VI, Vec3)>) -> &mut Self {
		self.morphs.push(offsets);
		self
	}
	
	pub fn build(self, renderer: &mut Renderer) -> Result<MMDModelShared<VI>, ModelError> {
		let (vertices, vertices_promise) = ImmutableBuffer::from_iter(self.vertices.into_iter(),
		                                                              BufferUsage{ vertex_buffer: true, ..BufferUsage::none() },
		                                                              renderer.load_queue.clone())?;
		
		let (indices, indices_promise) = ImmutableBuffer::from_iter(self.indices.into_iter(),
		                                                            BufferUsage{ index_buffer: true, ..BufferUsage::none() },
		                                                            renderer.load_queue.clone())?;
		
		let (default_tex, default_tex_promise) = {
			let texture_reader = Cursor::new(&include_bytes!("./default_tex.png")[..]);
			let image = image::load(texture_reader, ImageFormat::Png)?;
			let width = image.width();
			let height = image.height();
			
			ImmutableImage::from_iter(image.into_pre_mul_iter(),
			                          Dimensions::Dim2d{ width, height },
			                          MipmapsCount::Log2,
			                          Format::R8G8B8A8Unorm,
			                          renderer.load_queue.clone())?
		};
		
		let mut images_promise = default_tex_promise.boxed();
		let mut images = vec![];
		
		for texture in self.textures {
			let width = texture.width();
			let height = texture.height();
			
			let (image, promise) = ImmutableImage::from_iter(texture.into_pre_mul_iter(),
			                                                 Dimensions::Dim2d{ width, height },
			                                                 MipmapsCount::Log2,
			                                                 Format::R8G8B8A8Unorm,
			                                                 renderer.load_queue.clone())?;
			
			images_promise = images_promise.join(promise).boxed();
			images.push(image);
		}
		
		let mut material_promises = vulkano::sync::now(renderer.device.clone()).boxed();
		let mut sub_meshes = vec![];
		
		for desc in self.sub_meshes {
			let indices = indices.clone()
			                     .into_buffer_slice()
			                     .slice(desc.range.clone())
			                     .ok_or(ModelError::IndicesRangeError(desc.range, indices.len()))?;
			
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
				color: desc.color,
				specular: desc.specular,
				specularity: desc.specularity,
				ambient: desc.ambient,
				sphere_mode: desc.sphere_mode,
			};
			
			let (material_buffer, material_promise) = ImmutableBuffer::from_data(material_info,
			                                                                     BufferUsage{ uniform_buffer: true, ..BufferUsage::none() },
			                                                                     renderer.load_queue.clone())?;
			
			let sub_mesh = SubMesh::new(indices, material_buffer, texture, toon, sphere_map, desc.opaque, desc.no_cull, desc.edge, renderer)?;
			
			material_promises = material_promises.join(material_promise).boxed();
			sub_meshes.push(sub_mesh);
		}
		
		let default_bones = self.bones;
		let morphs_sizes = self.morphs.iter()
		                              .map(|v| v.len())
		                              .collect::<Vec<_>>();
		
		let (morphs_offsets, morphs_promise) = {
			let morph_max_size = morphs_sizes.iter().copied().max().unwrap_or(MORPH_GROUP_SIZE);
			let morph_max_size = (morph_max_size + MORPH_GROUP_SIZE - 1) / MORPH_GROUP_SIZE * MORPH_GROUP_SIZE;
			let morphs_count = self.morphs.len();
			
			let mut offsets = vec![IVec4::zeros(); morph_max_size * morphs_count];
			
			for (mid, morph) in self.morphs.into_iter().enumerate() {
				for (oid, (index, offset)) in morph.into_iter().enumerate() {
					offsets[mid + oid * morphs_count] = IVec4::new((offset.x * 1_000_000.0) as i32,
					                                               (offset.y * 1_000_000.0) as i32,
					                                               (offset.z * 1_000_000.0) as i32,
					                                               index.into());
				}
			}
			
			ImmutableBuffer::from_iter(offsets.into_iter(),
			                           BufferUsage{ storage_buffer: true, uniform_buffer: true, ..BufferUsage::none() },
			                           renderer.load_queue.clone())?
		};
		
		let bones_pool = CpuBufferPool::upload(renderer.load_queue.device().clone());
		let morphs_pool = CpuBufferPool::upload(renderer.load_queue.device().clone());
		
		let morphs_pipeline = renderer.pipelines.get()?;
		
		let fence = FenceCheck::new(vertices_promise.join(indices_promise)
		                                            .join(images_promise)
		                                            .join(material_promises)
		                                            .join(morphs_promise))?;
		
		Ok(MMDModelShared {
			vertices,
			sub_meshes,
			default_bones,
			bones_pool,
			morphs_offsets,
			morphs_sizes,
			morphs_pool,
			morphs_pipeline,
			fence,
		})
	}
}

pub struct SubMeshDesc {
	pub range: Range<usize>,
	pub texture: Option<usize>,
	pub toon: Option<usize>,
	pub sphere_map: Option<usize>,
	pub color: [f32; 4],
	pub specular: [f32; 3],
	pub specularity: f32,
	pub ambient: [f32; 3],
	pub sphere_mode: u32,
	pub no_cull: bool,
	pub opaque: bool,
	pub edge: Option<(f32, [f32; 4])>,
}

