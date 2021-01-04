use std::sync::Arc;
use vulkano::pipeline::GraphicsPipelineAbstract;

use crate::renderer::pipelines::{MMDPipeline, PipelineError};
use crate::renderer::pipelines::mmd::culling::{Cull, NoCull};
use crate::renderer::Renderer;

#[derive(Clone)]
pub enum Pipeline {
	Cull(Arc<MMDPipeline<Cull>>),
	NoCull(Arc<MMDPipeline<NoCull>>),
}

impl Pipeline {
	pub fn get(renderer: &mut Renderer, no_cull: bool) -> Result<Self, PipelineError> {
		Ok(match (no_cull, ) {
			(false, ) => Pipeline::Cull(renderer.pipelines.get::<MMDPipeline<Cull>>()?),
			(true, ) => Pipeline::NoCull(renderer.pipelines.get::<MMDPipeline<NoCull>>()?),
		})
	}
	
	pub fn as_abstract(&self) -> Arc<dyn GraphicsPipelineAbstract + Send + Sync> {
		match self {
			Pipeline::Cull(arc) => arc.clone(),
			Pipeline::NoCull(arc) => arc.clone(),
		}
	}
}
