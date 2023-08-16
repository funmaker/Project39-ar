use std::cell::Cell;
use std::sync::Arc;
use egui::Ui;
use vulkano::pipeline::{Pipeline, GraphicsPipeline, PipelineBindPoint};

use crate::application::{Application, Entity};
use crate::math::{Similarity3, Color};
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::renderer::pipelines::PipelineError;
use crate::renderer::pipelines::default::{DefaultGlowPipeline, GlowPc};
use crate::utils::{AutoCommandBufferBuilderEx, ExUi};
use super::{Component, ComponentBase, ComponentInner, ComponentError};
use super::model::SimpleModel;


#[derive(ComponentBase)]
pub struct Glow {
	#[inner] inner: ComponentInner,
	pipeline: Arc<GraphicsPipeline>,
	color: Cell<Color>,
	size: Cell<f32>,
}

impl Glow {
	pub fn new(color: Color, size: f32, renderer: &mut Renderer) -> Result<Self, PipelineError> {
		let pipeline = renderer.pipelines.get::<DefaultGlowPipeline>()?;
		
		Ok(Glow {
			inner: ComponentInner::from_render_type(RenderType::Opaque),
			pipeline,
			color: Cell::new(color),
			size: Cell::new(size),
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
	
	fn render(&self, entity: &Entity, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<(), ComponentError> {
		let pos = Similarity3::from_isometry(*entity.state().position, -1.0);
		
		if let Some(model) = entity.find_component_by_type::<SimpleModel>() {
			context.builder.bind_pipeline_graphics(self.pipeline.clone())
			               .bind_vertex_buffers(0, model.vertices.clone())
			               .bind_any_index_buffer(model.indices.clone())
			               .bind_descriptor_sets(PipelineBindPoint::Graphics,
			                                     self.pipeline.layout().clone(),
			                                     0,
			                                     model.set.clone())
			               .push_constants(self.pipeline.layout().clone(),
			                               0,
			                               GlowPc {
				                               model: pos.to_homogeneous().into(),
				                               color: self.color.get().into(),
				                               scale: self.size.get(),
			                               })
			               .draw_indexed(model.indices.len() as u32,
			                             1,
			                             0,
			                             0,
			                             0)?;
		}
		
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, _application: &Application) {
		ui.inspect_row("Color", &self.color, ());
		ui.inspect_row("Size", &self.size, (0.0001, 0.0..=0.1));
	}
}
