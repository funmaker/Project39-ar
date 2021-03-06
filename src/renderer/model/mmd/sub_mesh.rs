use std::sync::Arc;
use vulkano::buffer::{ImmutableBuffer, BufferSlice};
use vulkano::image::{ImmutableImage, view::ImageView};
use vulkano::sampler::Sampler;
use vulkano::descriptor::{DescriptorSet};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;

use crate::renderer::pipelines::mmd::{MMDPipelineOpaqueNoCull, MMDPipelineOpaque, MMDPipelineTransNoCull, MMDPipelineTrans, MMDPipelineOutline, MMDPipelineAny};
use crate::renderer::Renderer;
use crate::renderer::model::{ModelError, VertexIndex};
use vulkano::pipeline::GraphicsPipelineAbstract;

pub type PipelineWithSet = (MMDPipelineAny, Arc<dyn DescriptorSet + Send + Sync>);

#[derive(Debug, Copy, Clone)]
pub struct MaterialInfo {
	pub color: [f32; 4],
	pub specular: [f32; 3],
	pub specularity: f32,
	pub ambient: [f32; 3],
	pub sphere_mode: u32,
}

pub struct SubMesh<VI: VertexIndex> {
	pub indices: BufferSlice<[VI], Arc<ImmutableBuffer<[VI]>>>,
	pub main: PipelineWithSet,
	pub transparent: Option<PipelineWithSet>,
	pub edge: Option<PipelineWithSet>,
	pub edge_scale: f32,
	pub edge_color: [f32; 4],
}

impl<VI: VertexIndex> SubMesh<VI> {
	pub fn new(indices: BufferSlice<[VI], Arc<ImmutableBuffer<[VI]>>>,
	           material_buffer: Arc<ImmutableBuffer<MaterialInfo>>,
	           texture: Arc<ImmutableImage>,
	           toon: Arc<ImmutableImage>,
	           sphere_map: Arc<ImmutableImage>,
	           opaque: bool,
	           no_cull: bool,
	           edge: Option<(f32, [f32; 4])>,
	           renderer: &mut Renderer)
	           -> Result<SubMesh<VI>, ModelError> {
		let sampler = Sampler::simple_repeat_linear(renderer.device.clone());
		
		let main_pipeline: MMDPipelineAny = match no_cull {
			false => renderer.pipelines.get::<MMDPipelineOpaque>()?.into(),
			true  => renderer.pipelines.get::<MMDPipelineOpaqueNoCull>()?.into(),
		};
		
		let texture_view = ImageView::new(texture)?;
		let toon_view = ImageView::new(toon)?;
		let sphere_map_view = ImageView::new(sphere_map)?;
		
		let main_set = Arc::new(
			PersistentDescriptorSet::start(main_pipeline.layout().descriptor_set_layout(1).ok_or(ModelError::NoLayout)?.clone())
				.add_buffer(material_buffer.clone())?
				.add_sampled_image(texture_view.clone(), sampler.clone())?
				.add_sampled_image(toon_view.clone(), sampler.clone())?
				.add_sampled_image(sphere_map_view.clone(), sampler.clone())?
				.build()?
		);
		
		let mut sub_mesh = SubMesh {
			indices,
			main: (main_pipeline, main_set),
			transparent: None,
			edge: None,
			edge_scale: 0.0,
			edge_color: [0.0, 0.0, 0.0, 0.0],
		};
		
		if !opaque {
			let pipeline: MMDPipelineAny = match no_cull {
				false => renderer.pipelines.get::<MMDPipelineTrans>()?.into(),
				true  => renderer.pipelines.get::<MMDPipelineTransNoCull>()?.into(),
			};
			
			let set = Arc::new(
				PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layout(1).ok_or(ModelError::NoLayout)?.clone())
					.add_buffer(material_buffer.clone())?
					.add_sampled_image(texture_view.clone(), sampler.clone())?
					.add_sampled_image(toon_view.clone(), sampler.clone())?
					.add_sampled_image(sphere_map_view.clone(), sampler.clone())?
					.build()?
			);
			
			sub_mesh.transparent = Some((pipeline, set));
		}
		
		if let Some((scale, color)) = edge {
			sub_mesh.edge_scale = scale;
			sub_mesh.edge_color = color;
			
			let pipeline = renderer.pipelines.get::<MMDPipelineOutline>()?;
			
			let set = Arc::new(
				PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layout(1).ok_or(ModelError::NoLayout)?.clone())
					.add_sampled_image(texture_view.clone(), sampler.clone())?
					.build()?
			);
			
			sub_mesh.edge = Some((pipeline.into(), set));
		}
		
		Ok(sub_mesh)
	}
}

