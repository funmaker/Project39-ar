use std::sync::Arc;
use std::ops::Range;
use vulkano::buffer::ImmutableBuffer;
use vulkano::image::ImmutableImage;
use vulkano::format::Format;
use vulkano::sampler::Sampler;
use vulkano::descriptor::{DescriptorSet, PipelineLayoutAbstract};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;

use crate::renderer::pipelines::mmd::{MMDPipelineOpaqueNoCull, MMDPipelineOpaque, MMDPipelineTransNoCull, MMDPipelineTrans, MMDPipelineOutline, MMDPipelineAny};
use crate::renderer::Renderer;
use crate::renderer::model::ModelError;

pub type PipelineWithSet = (MMDPipelineAny, Arc<dyn DescriptorSet + Send + Sync>);

pub struct MaterialInfo {
	pub color: [f32; 4],
	pub specular: [f32; 3],
	pub specularity: f32,
	pub ambient: [f32; 3],
	pub sphere_mode: u32,
}

pub struct SubMesh {
	pub main: PipelineWithSet,
	pub transparent: Option<PipelineWithSet>,
	pub edge: Option<PipelineWithSet>,
	pub edge_scale: f32,
	pub edge_color: [f32; 4],
	pub range: Range<usize>,
}

impl SubMesh {
	pub fn new(range: Range<usize>,
	           material_buffer: Arc<ImmutableBuffer<MaterialInfo>>,
	           texture: Arc<ImmutableImage<Format>>,
	           toon: Arc<ImmutableImage<Format>>,
	           sphere_map: Arc<ImmutableImage<Format>>,
	           opaque: bool,
	           no_cull: bool,
	           edge: Option<(f32, [f32; 4])>,
	           renderer: &mut Renderer)
	           -> Result<SubMesh, ModelError> {
		let sampler = Sampler::simple_repeat_linear_no_mipmap(renderer.device.clone());
		
		let main_pipeline: MMDPipelineAny = match no_cull {
			false => renderer.pipelines.get::<MMDPipelineOpaque>()?.into(),
			true  => renderer.pipelines.get::<MMDPipelineOpaqueNoCull>()?.into(),
		};
		
		let main_set = Arc::new(
			PersistentDescriptorSet::start(main_pipeline.descriptor_set_layout(0).ok_or(ModelError::NoLayout)?
				.clone())
				.add_buffer(renderer.commons.clone())?
				.add_buffer(material_buffer.clone())?
				.add_sampled_image(texture.clone(), sampler.clone())?
				.add_sampled_image(toon.clone(), sampler.clone())?
				.add_sampled_image(sphere_map.clone(), sampler.clone())?
				.build()?
		);
		
		let mut sub_mesh = SubMesh {
			main: (main_pipeline, main_set),
			transparent: None,
			edge: None,
			range,
			edge_scale: 0.0,
			edge_color: [0.0, 0.0, 0.0, 0.0],
		};
		
		if !opaque {
			let pipeline: MMDPipelineAny = match no_cull {
				false => renderer.pipelines.get::<MMDPipelineTrans>()?.into(),
				true  => renderer.pipelines.get::<MMDPipelineTransNoCull>()?.into(),
			};
			
			let set = Arc::new(
				PersistentDescriptorSet::start(pipeline.descriptor_set_layout(0).ok_or(ModelError::NoLayout)?
					.clone())
					.add_buffer(renderer.commons.clone())?
					.add_buffer(material_buffer.clone())?
					.add_sampled_image(texture.clone(), sampler.clone())?
					.add_sampled_image(toon.clone(), sampler.clone())?
					.add_sampled_image(sphere_map.clone(), sampler.clone())?
					.build()?
			);
			
			sub_mesh.transparent = Some((pipeline, set));
		}
		
		if let Some((scale, color)) = edge {
			sub_mesh.edge_scale = scale;
			sub_mesh.edge_color = color;
			
			let pipeline = renderer.pipelines.get::<MMDPipelineOutline>()?;
			
			let set = Arc::new(
				PersistentDescriptorSet::start(pipeline.descriptor_set_layout(0).ok_or(ModelError::NoLayout)?
					.clone())
					.add_buffer(renderer.commons.clone())?
					.add_sampled_image(texture.clone(), sampler.clone())?
					.build()?
			);
			
			sub_mesh.edge = Some((pipeline.into(), set));
		}
		
		Ok(sub_mesh)
	}
}

