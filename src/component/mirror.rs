use std::cell::{Cell, RefCell};
use std::sync::Arc;
use image::{DynamicImage, GenericImageView};
use vulkano::image::{ImmutableImage, ImageDimensions, MipmapsCount};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::{DescriptorSet, PersistentDescriptorSet};
use vulkano::format::Format;
use vulkano::sync::GpuFuture;
use vulkano::buffer::{BufferUsage, ImmutableBuffer, TypedBufferAccess};
use vulkano::image::view::ImageView;
use vulkano::pipeline::{GraphicsPipeline, PipelineBindPoint};
use vulkano::sampler::Sampler;

use crate::application::Entity;
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::renderer::pipelines::mirror::{MirrorPipeline, Vertex};
use crate::utils::{ImageEx, FenceCheck};
use crate::renderer::Renderer;

#[derive(ComponentBase)]
pub struct Mirror {
	#[inner] inner: ComponentInner,
	enabled: Cell<bool>,
	pipeline: Arc<GraphicsPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	fence: RefCell<FenceCheck>,
	set: RefCell<Option<Arc<dyn DescriptorSet + Send + Sync>>>,
}

impl Mirror {
	pub fn new(renderer: &mut Renderer) -> Self {
		let pipeline = renderer.pipelines.get::<MirrorPipeline>().unwrap();
		
		let square = [
			Vertex::new([-1.0, -1.0, 0.5], [0.0, 0.0], [0.5, 0.0]),
			Vertex::new([-1.0,  1.0, 0.5], [0.0, 1.0], [0.5, 1.0]),
			Vertex::new([ 1.0, -1.0, 0.5], [0.5, 0.0], [1.0, 0.0]),
			Vertex::new([ 1.0, -1.0, 0.5], [0.5, 0.0], [1.0, 0.0]),
			Vertex::new([-1.0,  1.0, 0.5], [0.0, 1.0], [0.5, 1.0]),
			Vertex::new([ 1.0,  1.0, 0.5], [0.5, 1.0], [1.0, 1.0]),
		];
		
		let (vertices, vertices_promise) = ImmutableBuffer::from_iter(square.iter().cloned(),
		                                                              BufferUsage{ vertex_buffer: true, ..BufferUsage::none() },
		                                                              renderer.queue.clone()).unwrap();
		
		let fence = RefCell::new(FenceCheck::new(vertices_promise).unwrap());
		
		Mirror {
			inner: ComponentInner::new(),
			enabled: Cell::new(false),
			pipeline,
			vertices,
			fence,
			set: RefCell::new(None),
		}
	}
	
	pub fn set_enabled(&self, enabled: bool) {
		self.enabled.set(enabled);
	}
	
	pub fn set_image(&self, image: Arc<DynamicImage>, renderer: &Renderer) {
		let width = image.width();
		let height = image.height();
		
		let (image, image_promise) = ImmutableImage::from_iter((*image).clone().into_pre_mul_iter(),
		                                                       ImageDimensions::Dim2d{ width, height, array_layers: 1 },
		                                                       MipmapsCount::Log2,
		                                                       Format::R8G8B8A8_UNORM,
		                                                       renderer.queue.clone()).unwrap();
		
		let set = {
			let mut set_builder = PersistentDescriptorSet::start(self.pipeline.layout().descriptor_set_layouts().get(0).unwrap().clone());
			set_builder.add_sampled_image(ImageView::new(image.clone()).unwrap(), Sampler::simple_repeat_linear(renderer.device.clone())).unwrap();
			Arc::new(set_builder.build().unwrap())
		};
		
		self.set.replace(Some(set));
		
		let mut fence = self.fence.borrow_mut();
		*fence = FenceCheck::new(fence.future().join(image_promise)).unwrap();
	}
}

impl Component for Mirror {
	fn render(&self, _entity: &Entity, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), ComponentError> {
		if !self.enabled.get() || !self.fence.borrow().check() || self.set.borrow().is_none() {
			return Ok(());
		}
		
		builder.bind_pipeline_graphics(self.pipeline.clone())
		       .bind_vertex_buffers(0, self.vertices.clone())
		       .bind_descriptor_sets(PipelineBindPoint::Graphics,
		                             self.pipeline.layout().clone(),
		                             0,
		                             self.set.borrow().clone().unwrap())
		       .draw(self.vertices.len() as u32,
		                     1,
		                     0,
		                     0)?;
		
		Ok(())
	}
}
