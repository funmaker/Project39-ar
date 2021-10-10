use std::sync::Arc;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::pipeline::{GraphicsPipeline, PipelineBindPoint};

use crate::application::Entity;
use crate::renderer::Renderer;
use crate::math::{Similarity3, Color};
use crate::renderer::pipelines::default::DefaultGlowPipeline;
use crate::renderer::pipelines::PipelineError;
use crate::utils::AutoCommandBufferBuilderEx;
use crate::component::model::SimpleModel;
use super::{Component, ComponentBase, ComponentInner, ComponentError};

#[derive(ComponentBase)]
pub struct Glow {
	#[inner] inner: ComponentInner,
	pipeline: Arc<GraphicsPipeline>,
	color: Color,
}

impl Glow {
	pub fn new(color: Color, renderer: &mut Renderer) -> Result<Self, PipelineError> {
		let pipeline = renderer.pipelines.get::<DefaultGlowPipeline>()?;
		
		Ok(Glow {
			inner: ComponentInner::new(),
			pipeline,
			color,
		})
	}
}

impl Component for Glow {
	// fn start(&self, entity: &Entity, _application: &Application) -> Result<(), ComponentError> {
	// 	if let Some(model) = AnyModel::find(entity) {
	// 		*self.model.borrow_mut() = Some(model);
	// 	} else {
	// 		self.remove();
	// 		println!("Model not found");
	// 	}
	//
	// 	Ok(())
	// }
	
	fn render(&self, entity: &Entity, _renderer: &Renderer, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		let pos = Similarity3::from_isometry(entity.state().position, 1.0);
		
		if let Some(model) = entity.find_component_by_type::<SimpleModel>() {
			builder.bind_pipeline_graphics(self.pipeline.clone())
			       .bind_vertex_buffers(0, model.vertices.clone())
			       .bind_any_index_buffer(model.indices.clone())
			       .bind_descriptor_sets(PipelineBindPoint::Graphics,
			                             self.pipeline.layout().clone(),
			                             0,
			                             model.set.clone())
			       .push_constants(self.pipeline.layout().clone(),
			                       0,
			                       (pos.to_homogeneous(), self.color, 0.01_f32))
			       .draw_indexed(model.indices.len() as u32,
			                     1,
			                     0,
			                     0,
			                     0)?;
		}
		
		Ok(())
	}
}
