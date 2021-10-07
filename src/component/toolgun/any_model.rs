use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};

use crate::component::ComponentError;
use crate::component::model::SimpleModel;
use crate::application::Entity;
use crate::math::{Similarity3, Color};

pub enum AnyModel {
	Simple16(SimpleModel<u16>),
	Simple32(SimpleModel<u32>),
}

impl AnyModel {
	pub fn find(entity: &Entity) -> Option<Self> {
		if let Some(model) = entity.find_component_by_type::<SimpleModel<u16>>() {
			Some(AnyModel::Simple16(model.clone()))
		} else if let Some(model) = entity.find_component_by_type::<SimpleModel<u32>>() {
			Some(AnyModel::Simple32(model.clone()))
		} else {
			None
		}
	}
	
	pub fn render_impl(&self, transform: Similarity3, color: Color, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		match self {
			AnyModel::Simple16(model) => model.render_impl(transform, color, builder),
			AnyModel::Simple32(model) => model.render_impl(transform, color, builder),
		}
	}
}
