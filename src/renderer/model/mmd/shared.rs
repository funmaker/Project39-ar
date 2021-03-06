use std::sync::Arc;
use std::ops::Range;
use std::io::Cursor;
use image::{DynamicImage, GenericImageView, ImageFormat};
use vulkano::buffer::{ImmutableBuffer, BufferUsage, CpuBufferPool, TypedBufferAccess};
use vulkano::image::{ImmutableImage, Dimensions, MipmapsCount};
use vulkano::sync::GpuFuture;
use vulkano::format::Format;
use vulkano::descriptor::descriptor_set::UnsafeDescriptorSetLayout;

use crate::application::entity::Bone;
use crate::renderer::model::{ModelError, VertexIndex, FenceCheck};
use crate::renderer::pipelines::mmd::MMDPipelineOpaque;
use crate::renderer::Renderer;
use crate::utils::ImageEx;
use crate::math::{AMat4, Vec3, Vec4};
use super::sub_mesh::{SubMesh, MaterialInfo};
use super::Vertex;
use vulkano::descriptor::PipelineLayoutAbstract;

pub struct MMDModelShared<VI: VertexIndex> {
	pub vertices: Arc<ImmutableBuffer<[Vertex]>>,
	pub indices: Arc<ImmutableBuffer<[VI]>>,
	pub sub_mesh: Vec<SubMesh>,
	pub default_bones: Vec<Bone>,
	pub fences: Vec<FenceCheck>,
	pub bones_pool: CpuBufferPool<AMat4>,
	pub morphs_count: usize,
	pub shapekeys: Option<Arc<ImmutableBuffer<[Vec4]>>>,
	pub morphs_pool: CpuBufferPool<f32>,
	default_tex: Option<Arc<ImmutableImage<Format>>>,
}

impl<VI: VertexIndex> MMDModelShared<VI> {
	pub fn new(vertices: &[Vertex], indices: &[VI], renderer: &mut Renderer) -> Result<MMDModelShared<VI>, ModelError> {
		let queue = &renderer.load_queue;
		
		let (vertices, vertices_promise) = ImmutableBuffer::from_iter(vertices.iter().cloned(),
		                                                              BufferUsage{ vertex_buffer: true, ..BufferUsage::none() },
		                                                              queue.clone())?;
		
		let (indices, indices_promise) = ImmutableBuffer::from_iter(indices.iter().copied(),
		                                                            BufferUsage{ index_buffer: true, ..BufferUsage::none() },
		                                                            queue.clone())?;
		
		let fences = vec![FenceCheck::new(vertices_promise.join(indices_promise))?];
		
		let bones_pool = CpuBufferPool::upload(queue.device().clone());
		let morphs_pool = CpuBufferPool::upload(queue.device().clone());
		
		Ok(MMDModelShared {
			vertices,
			indices,
			sub_mesh: vec![],
			fences,
			default_bones: vec![],
			default_tex: None,
			morphs_count: 0,
			shapekeys: None,
			morphs_pool,
			bones_pool,
		})
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
	
	pub fn get_default_tex(&mut self, renderer: &mut Renderer) -> Result<Arc<ImmutableImage<Format>>, ModelError> {
		if let Some(image) = self.default_tex.clone() {
			return Ok(image);
		}
		
		let texture_reader = Cursor::new(&include_bytes!("./default_tex.png")[..]);
		let image = image::load(texture_reader, ImageFormat::Png)?;
		let texture = self.add_texture(image, renderer)?;
		
		self.default_tex = Some(texture.clone());
		
		Ok(texture)
	}
	
	pub fn add_bone(&mut self, bone: Bone) {
		self.default_bones.push(bone);
	}
	
	pub fn add_morphs(&mut self, morphs: &[Vec<(VI, Vec3)>], renderer: &mut Renderer) -> Result<(), ModelError> {
		let vertices = self.vertices.len();
		let mut morphs_vec = vec![Vec4::zeros(); vertices * morphs.len()];
		
		for (mid, morph) in morphs.iter().enumerate() {
			for &(oid, ref offset) in morph {
				let oid: usize = oid.into();
				morphs_vec[mid * vertices + oid] = offset.to_homogeneous();
			}
		}
		
		let (buffer, promise) = ImmutableBuffer::from_iter(morphs_vec.into_iter(),
		                                                   BufferUsage{ uniform_buffer: true, ..BufferUsage::none() },
		                                                   renderer.load_queue.clone())?;
		
		self.shapekeys = Some(buffer);
		self.morphs_count = morphs.len();
		self.fences.push(FenceCheck::new(promise)?);
		
		Ok(())
	}
	
	pub fn commons_layout(&self, renderer: &mut Renderer) -> Result<Arc<UnsafeDescriptorSetLayout>, ModelError> {
		self.sub_mesh.first()
			.map(|mesh| mesh.main.0.clone())
			.ok_or(ModelError::NoLayout)
			.or_else(|_| renderer.pipelines.get::<MMDPipelineOpaque>().map_err(Into::into).map(Into::into))
			.and_then(|pipeline| pipeline.descriptor_set_layout(0).cloned().ok_or(ModelError::NoLayout))
	}
}
