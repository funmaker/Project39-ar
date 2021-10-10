use std::sync::Arc;
use std::ops::Range;
use vulkano::buffer::ImmutableBuffer;
use vulkano::image::{ImmutableImage, view::ImageView};
use vulkano::sampler::Sampler;
use vulkano::descriptor_set::{DescriptorSet, PersistentDescriptorSet};
use vulkano::pipeline::GraphicsPipeline;

use crate::renderer::pipelines::mmd::{MMDPipelineOpaqueNoCull, MMDPipelineOpaque, MMDPipelineTransNoCull, MMDPipelineTrans, MMDPipelineOutline};
use crate::renderer::Renderer;
use crate::component::model::ModelError;

pub type PipelineWithSet = (Arc<GraphicsPipeline>, Arc<dyn DescriptorSet + Send + Sync>);

#[derive(Debug, Copy, Clone)]
pub struct MaterialInfo {
	pub color: [f32; 4],
	pub specular: [f32; 3],
	pub specularity: f32,
	pub ambient: [f32; 3],
	pub sphere_mode: u32,
}

pub struct SubMesh {
	pub range: Range<u32>,
	pub main: PipelineWithSet,
	pub transparent: Option<PipelineWithSet>,
	pub edge: Option<PipelineWithSet>,
	pub edge_scale: f32,
	pub edge_color: [f32; 4],
}

impl SubMesh {
	pub fn new(range: Range<u32>,
	           material_buffer: Arc<ImmutableBuffer<MaterialInfo>>,
	           texture: Arc<ImmutableImage>,
	           toon: Arc<ImmutableImage>,
	           sphere_map: Arc<ImmutableImage>,
	           opaque: bool,
	           no_cull: bool,
	           edge: Option<(f32, [f32; 4])>,
	           renderer: &mut Renderer)
	           -> Result<SubMesh, ModelError> {
		let sampler = Sampler::simple_repeat_linear(renderer.device.clone());
		
		let main_pipeline = match no_cull {
			false => renderer.pipelines.get::<MMDPipelineOpaque>()?,
			true  => renderer.pipelines.get::<MMDPipelineOpaqueNoCull>()?,
		};
		
		let texture_view = ImageView::new(texture)?;
		let toon_view = ImageView::new(toon)?;
		let sphere_map_view = ImageView::new(sphere_map)?;
		
		let main_set = {
			let mut set_builder = PersistentDescriptorSet::start(main_pipeline.layout().descriptor_set_layouts().get(1).ok_or(ModelError::NoLayout)?.clone());
			set_builder.add_buffer(material_buffer.clone())?
			           .add_sampled_image(texture_view.clone(), sampler.clone())?
			           .add_sampled_image(toon_view.clone(), sampler.clone())?
			           .add_sampled_image(sphere_map_view.clone(), sampler.clone())?;
			Arc::new(set_builder.build()?)
		};
		
		let mut sub_mesh = SubMesh {
			range,
			main: (main_pipeline, main_set),
			transparent: None,
			edge: None,
			edge_scale: 0.0,
			edge_color: [0.0, 0.0, 0.0, 0.0],
		};
		
		if !opaque {
			let pipeline = match no_cull {
				false => renderer.pipelines.get::<MMDPipelineTrans>()?,
				true  => renderer.pipelines.get::<MMDPipelineTransNoCull>()?,
			};
			
			let set = {
				let mut set_builder = PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layouts().get(1).ok_or(ModelError::NoLayout)?.clone());
				set_builder.add_buffer(material_buffer.clone())?
				           .add_sampled_image(texture_view.clone(), sampler.clone())?
				           .add_sampled_image(toon_view.clone(), sampler.clone())?
				           .add_sampled_image(sphere_map_view.clone(), sampler.clone())?;
				Arc::new(set_builder.build()?)
			};
			
			sub_mesh.transparent = Some((pipeline, set));
		}
		
		if let Some((scale, color)) = edge {
			sub_mesh.edge_scale = scale;
			sub_mesh.edge_color = color;
			
			let pipeline = renderer.pipelines.get::<MMDPipelineOutline>()?;
			
			let set = {
				let mut set_builder = PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layouts().get(1).ok_or(ModelError::NoLayout)?.clone());
				set_builder.add_sampled_image(texture_view.clone(), sampler.clone())?;
				Arc::new(set_builder.build()?)
			};
			
			sub_mesh.edge = Some((pipeline.into(), set));
		}
		
		Ok(sub_mesh)
	}
}

