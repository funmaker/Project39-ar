use std::sync::Arc;
use std::ops::Range;
use bytemuck::{Pod, Zeroable};
use vulkano::buffer::DeviceLocalBuffer;
use vulkano::image::{ImmutableImage, view::ImageView};
use vulkano::sampler::{Sampler, SamplerCreateInfo};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::pipeline::{GraphicsPipeline, Pipeline};

use crate::component::model::mmd::pipeline::{MMDPipelineOpaqueNoCull, MMDPipelineOpaque, MMDPipelineTransNoCull, MMDPipelineTrans, MMDPipelineOutline};
use crate::renderer::Renderer;
use crate::component::model::ModelError;
use crate::math::{Vec3, Vec4};
use crate::utils::NgPod;

pub type PipelineWithSet = (Arc<GraphicsPipeline>, Arc<PersistentDescriptorSet>);

#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
pub struct MaterialInfo {
	pub color: NgPod<Vec4>,
	pub specular: NgPod<Vec3>,
	pub specularity: f32,
	pub ambient: NgPod<Vec3>,
	pub sphere_mode: u32,
}

pub struct SubMesh {
	pub range: Range<u32>,
	pub main: PipelineWithSet,
	pub transparent: Option<PipelineWithSet>,
	pub edge: Option<PipelineWithSet>,
	pub edge_scale: f32,
	pub edge_color: Vec4,
}

impl SubMesh {
	pub fn new(range: Range<u32>,
	           material_buffer: Arc<DeviceLocalBuffer<MaterialInfo>>,
	           texture: Arc<ImmutableImage>,
	           toon: Arc<ImmutableImage>,
	           sphere_map: Arc<ImmutableImage>,
	           opaque: bool,
	           no_cull: bool,
	           edge: Option<(f32, Vec4)>,
	           renderer: &mut Renderer)
	           -> Result<SubMesh, ModelError> {
		let sampler = Sampler::new(renderer.device.clone(), SamplerCreateInfo::simple_repeat_linear())?;
		
		let main_pipeline = match no_cull {
			false => renderer.pipelines.get::<MMDPipelineOpaque>()?,
			true  => renderer.pipelines.get::<MMDPipelineOpaqueNoCull>()?,
		};
		
		let texture_view = ImageView::new_default(texture)?;
		let toon_view = ImageView::new_default(toon)?;
		let sphere_map_view = ImageView::new_default(sphere_map)?;
		
		let main_set = PersistentDescriptorSet::new(&renderer.descriptor_set_allocator,
		                                            main_pipeline.layout().set_layouts().get(1).ok_or(ModelError::NoLayout)?.clone(), [
			                                            WriteDescriptorSet::buffer(0, material_buffer.clone()),
			                                            WriteDescriptorSet::image_view_sampler(1, texture_view.clone(), sampler.clone()),
			                                            WriteDescriptorSet::image_view_sampler(2, toon_view.clone(), sampler.clone()),
			                                            WriteDescriptorSet::image_view_sampler(3, sphere_map_view.clone(), sampler.clone()),
		                                            ])?;
		
		let mut sub_mesh = SubMesh {
			range,
			main: (main_pipeline, main_set),
			transparent: None,
			edge: None,
			edge_scale: 0.0,
			edge_color: vector![0.0, 0.0, 0.0, 0.0],
		};
		
		if !opaque {
			let pipeline = match no_cull {
				false => renderer.pipelines.get::<MMDPipelineTrans>()?,
				true  => renderer.pipelines.get::<MMDPipelineTransNoCull>()?,
			};
			
			let set = PersistentDescriptorSet::new(&renderer.descriptor_set_allocator,
			                                       pipeline.layout().set_layouts().get(1).ok_or(ModelError::NoLayout)?.clone(), [
				                                       WriteDescriptorSet::buffer(0, material_buffer.clone()),
				                                       WriteDescriptorSet::image_view_sampler(1, texture_view.clone(), sampler.clone()),
				                                       WriteDescriptorSet::image_view_sampler(2, toon_view.clone(), sampler.clone()),
				                                       WriteDescriptorSet::image_view_sampler(3, sphere_map_view.clone(), sampler.clone()),
			                                       ])?;
			
			sub_mesh.transparent = Some((pipeline, set));
		}
		
		if let Some((scale, color)) = edge {
			sub_mesh.edge_scale = scale;
			sub_mesh.edge_color = color;
			
			let pipeline = renderer.pipelines.get::<MMDPipelineOutline>()?;
			
			let set = PersistentDescriptorSet::new(&renderer.descriptor_set_allocator,
			                                       pipeline.layout().set_layouts().get(1).ok_or(ModelError::NoLayout)?.clone(), [
				                                       WriteDescriptorSet::image_view_sampler(0, texture_view.clone(), sampler.clone()),
			                                       ])?;
			
			sub_mesh.edge = Some((pipeline.into(), set));
		}
		
		Ok(sub_mesh)
	}
}

pub struct SubMeshDesc {
	pub range: Range<u32>,
	pub texture: Option<usize>,
	pub toon: Option<usize>,
	pub sphere_map: Option<usize>,
	pub color: Vec4,
	pub specular: Vec3,
	pub specularity: f32,
	pub ambient: Vec3,
	pub sphere_mode: u32,
	pub no_cull: bool,
	pub opaque: bool,
	pub edge: Option<(f32, Vec4)>,
}
