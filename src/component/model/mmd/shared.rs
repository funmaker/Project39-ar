use std::sync::Arc;
use std::ops::Range;
use std::io::Cursor;
use image::{DynamicImage, GenericImageView, ImageFormat};
use vulkano::buffer::{ImmutableBuffer, BufferUsage, CpuBufferPool};
use vulkano::image::{ImmutableImage, MipmapsCount, ImageDimensions};
use vulkano::sync::GpuFuture;
use vulkano::format::Format;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::pipeline::ComputePipeline;

use crate::component::model::{ModelError, VertexIndex};
use crate::renderer::pipelines::mmd::{MMDPipelineOpaque, MMDPipelineMorphs, MORPH_GROUP_SIZE};
use crate::renderer::Renderer;
use crate::utils::{VecFuture, ImageEx, FenceCheck, ImmutableIndexBuffer};
use crate::math::{AMat4, IVec4, Vec3};
use super::sub_mesh::{SubMesh, MaterialInfo};
use super::{Vertex, Bone};

pub struct MMDModelShared {
	pub vertices: Arc<ImmutableBuffer<[Vertex]>>,
	pub indices: ImmutableIndexBuffer,
	pub sub_meshes: Vec<SubMesh>,
	pub default_bones: Vec<Bone>,
	pub bones_pool: CpuBufferPool<AMat4>,
	pub morphs_offsets: Arc<ImmutableBuffer<[IVec4]>>,
	pub morphs_sizes: Vec<usize>,
	pub morphs_max_size: usize,
	pub morphs_pool: CpuBufferPool<IVec4>,
	pub morphs_pipeline: Arc<ComputePipeline>,
	pub fence: FenceCheck,
}

impl MMDModelShared {
	pub fn new<VI: VertexIndex>(vertices: Vec<Vertex>, indices: Vec<VI>) -> MMDModelSharedBuilder<VI> {
		MMDModelSharedBuilder::new(vertices, indices)
	}
	
	pub fn commons_layout(&self, renderer: &mut Renderer) -> Result<Arc<DescriptorSetLayout>, ModelError> {
		self.sub_meshes.first()
		               .map(|mesh| mesh.main.0.clone())
		               .ok_or(ModelError::NoLayout)
		               .or_else(|_| renderer.pipelines.get::<MMDPipelineOpaque>().map_err(Into::into).map(Into::into))
		               .and_then(|pipeline| pipeline.layout().descriptor_set_layouts().get(0).cloned().ok_or(ModelError::NoLayout))
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
	
	pub fn build(self, renderer: &mut Renderer) -> Result<MMDModelShared, ModelError> {
		let mut image_promises = VecFuture::new(renderer.device.clone());
		let mut buffer_promises = VecFuture::new(renderer.device.clone());
		
		let (vertices, vertices_promise) = ImmutableBuffer::from_iter(self.vertices.into_iter(),
		                                                              BufferUsage{ vertex_buffer: true, ..BufferUsage::none() },
		                                                              renderer.load_queue.clone())?;
		buffer_promises.push(vertices_promise);
		
		let (indices, indices_promise) = ImmutableBuffer::from_iter(self.indices.into_iter(),
		                                                            BufferUsage{ index_buffer: true, ..BufferUsage::none() },
		                                                            renderer.load_queue.clone())?;
		buffer_promises.push(indices_promise);
		
		let mut images = vec![];
		
		let (default_tex, default_tex_promise) = {
			let texture_reader = Cursor::new(&include_bytes!("default_tex.png")[..]);
			let image = image::load(texture_reader, ImageFormat::Png)?;
			let width = image.width();
			let height = image.height();
			
			ImmutableImage::from_iter(image.into_pre_mul_iter(),
			                          ImageDimensions::Dim2d{ width, height, array_layers: 1 },
			                          MipmapsCount::Log2,
			                          Format::R8G8B8A8_UNORM,
			                          renderer.load_queue.clone())?
		};
		image_promises.push(default_tex_promise);
		
		for texture in self.textures {
			let width = texture.width();
			let height = texture.height();
			
			let (image, promise) = ImmutableImage::from_iter(texture.into_pre_mul_iter(),
			                                                 ImageDimensions::Dim2d{ width, height, array_layers: 1 },
			                                                 MipmapsCount::Log2,
			                                                 Format::R8G8B8A8_UNORM,
			                                                 renderer.load_queue.clone())?;
			
			image_promises.push(promise);
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
				color: desc.color,
				specular: desc.specular,
				specularity: desc.specularity,
				ambient: desc.ambient,
				sphere_mode: desc.sphere_mode,
			};
			
			let (material_buffer, material_promise) = ImmutableBuffer::from_data(material_info,
			                                                                     BufferUsage{ uniform_buffer: true, ..BufferUsage::none() },
			                                                                     renderer.load_queue.clone())?;
			
			let sub_mesh = SubMesh::new(desc.range, material_buffer, texture, toon, sphere_map, desc.opaque, desc.no_cull, desc.edge, renderer)?;
			
			buffer_promises.push(material_promise);
			sub_meshes.push(sub_mesh);
		}
		
		let default_bones = self.bones;
		let morphs_sizes = self.morphs.iter()
		                              .map(|v| v.len())
		                              .collect::<Vec<_>>();
		let morphs_max_size = morphs_sizes.iter().copied().max().unwrap_or(MORPH_GROUP_SIZE);
		let morphs_max_size = (morphs_max_size + MORPH_GROUP_SIZE - 1) / MORPH_GROUP_SIZE * MORPH_GROUP_SIZE;
		
		let (morphs_offsets, morphs_promise) = {
			let mut offsets = vec![IVec4::zeros(); morphs_max_size * self.morphs.len()];
			
			for (mid, morph) in self.morphs.into_iter().enumerate() {
				for (oid, (index, offset)) in morph.into_iter().enumerate() {
					offsets[mid * morphs_max_size + oid] = vector!((offset.x * 1_000_000.0) as i32,
					                                                  (offset.y * 1_000_000.0) as i32,
					                                                  (offset.z * 1_000_000.0) as i32,
					                                                  Into::<u32>::into(index) as i32);
				}
			}
			
			ImmutableBuffer::from_iter(offsets.into_iter(),
			                           BufferUsage{ storage_buffer: true, uniform_buffer: true, ..BufferUsage::none() },
			                           renderer.load_queue.clone())?
		};
		buffer_promises.push(morphs_promise);
		
		let bones_pool = CpuBufferPool::upload(renderer.load_queue.device().clone());
		let morphs_pool = CpuBufferPool::upload(renderer.load_queue.device().clone());
		
		let morphs_pipeline = renderer.pipelines.get::<MMDPipelineMorphs>()?;
		
		let fence = FenceCheck::new(image_promises.join(buffer_promises))?;
		
		Ok(MMDModelShared {
			vertices,
			indices: indices.into(),
			sub_meshes,
			default_bones,
			bones_pool,
			morphs_offsets,
			morphs_sizes,
			morphs_max_size,
			morphs_pool,
			morphs_pipeline,
			fence,
		})
	}
}

pub struct SubMeshDesc {
	pub range: Range<u32>,
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

