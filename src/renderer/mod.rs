use std::sync::Arc;
use err_derive::Error;
use vulkano::{pipeline, device, instance, sync, framebuffer, command_buffer, swapchain, memory};
use vulkano::swapchain::{Swapchain, SurfaceTransform, PresentMode, FullscreenExclusive, Surface, CompositeAlpha};
use vulkano::instance::{Instance, InstanceExtensions, PhysicalDevice};
use vulkano::instance::debug::{DebugCallback, MessageSeverity, MessageType};
use vulkano::command_buffer::{AutoCommandBufferBuilder, SubpassContents};
use vulkano::device::{Device, DeviceExtensions, Features, Queue};
use vulkano::image::{SwapchainImage, ImageUsage};
use vulkano::buffer::{BufferUsage, DeviceLocalBuffer};
use vulkano::descriptor::descriptor_set;
use vulkano::framebuffer::RenderPassAbstract;
use vulkano::format::{ClearValue, Format};
use vulkano::sync::GpuFuture;

pub mod model;
pub mod eye;
pub mod window;
pub mod pipelines;

use crate::debug;
use crate::application::Entity;
use crate::math::{Vec3, Vec4, Isometry3, AMat4, PMat4};
use eye::{Eyes, EyeCreationError};
use window::{Window, WindowSwapchainRegenError, WindowRenderError};
use pipelines::Pipelines;
use model::ModelRenderError;

type RenderPass = dyn RenderPassAbstract + Send + Sync;

#[derive(Clone)]
pub struct CommonsUBO {
	projection: [PMat4; 2],
	view: [AMat4; 2],
	light_direction: [Vec4; 2],
	ambient: f32,
}

pub struct Renderer {
	pub instance: Arc<Instance>,
	pub commons: Arc<DeviceLocalBuffer<CommonsUBO>>,
	
	device: Arc<Device>,
	queue: Arc<Queue>,
	load_queue: Arc<Queue>,
	pipelines: Pipelines,
	eyes: Eyes,
	previous_frame_end: Option<Box<dyn GpuFuture>>,
}

impl Renderer {
	pub fn new(device: Option<usize>) -> Result<Renderer, RendererError> {
		let instance = Renderer::create_vulkan_instance()?;
		
		if debug::debug() {
			Renderer::install_debug_callbacks(&instance);
		}
		
		let physical = Renderer::create_physical_device(device, &instance)?;
		let (device, queue, load_queue) = Renderer::create_device(physical)?;
		let render_pass = Renderer::create_render_pass(&device)?;
		
		let eyes = Eyes::new(&queue, &render_pass)?;
		
		let previous_frame_end = Some(Box::new(sync::now(device.clone())) as Box<_>);
		
		let commons = DeviceLocalBuffer::new(device.clone(),
		                                     BufferUsage{ transfer_destination: true,
		                                                  uniform_buffer: true,
		                                                  ..BufferUsage::none() },
		                                     Some(queue.family()))?;
		
		let pipelines = Pipelines::new(render_pass, eyes.frame_buffer_size);
		
		Ok(Renderer {
			instance,
			commons,
			device,
			queue,
			load_queue,
			pipelines,
			eyes,
			previous_frame_end,
		})
	}
	
	fn create_vulkan_instance() -> Result<Arc<Instance>, RendererError> {
		dprintln!("List of Vulkan debugging layers available to use:");
		let layers = vulkano::instance::layers_list()?;
		for layer in layers {
			dprintln!("\t{}", layer.name());
		}
		
		let app_infos = vulkano::app_info_from_cargo_toml!();
		
		let extensions = vulkano_win::required_extensions()
		                              .union(&InstanceExtensions { ext_debug_utils: debug::debug(),
		                                                           ..InstanceExtensions::none() });
		
		let layers = if debug::debug() {
			// TODO: Get better GPU
			vec![/*"VK_LAYER_LUNARG_standard_validation"*/]
		} else {
			vec![]
		};
		
		Ok(Instance::new(Some(&app_infos), &extensions, layers)?)
	}
	
	fn install_debug_callbacks(instance: &Arc<Instance>) {
		let severity = MessageSeverity { error:       true,
			warning:     true,
			information: false,
			verbose:     true, };
		
		let ty = MessageType::all();
		
		let _debug_callback = DebugCallback::new(instance, severity, ty, |msg| {
			let severity = if msg.severity.error {
				"error"
			} else if msg.severity.warning {
				"warning"
			} else if msg.severity.information {
				"information"
			} else if msg.severity.verbose {
				"verbose"
			} else {
				panic!("no-impl");
			};
			
			let ty = if msg.ty.general {
				"general"
			} else if msg.ty.validation {
				"validation"
			} else if msg.ty.performance {
				"performance"
			} else {
				panic!("no-impl");
			};
			
			println!("{} {} {}: {}",
			         msg.layer_prefix,
			         ty,
			         severity,
			         msg.description);
		});
	}
	
	fn create_physical_device(device: Option<usize>, instance: &Arc<Instance>) -> Result<PhysicalDevice, RendererError> {
		dprintln!("Devices:");
		for device in PhysicalDevice::enumerate(&instance) {
			dprintln!("\t{}: {} api: {} driver: {}",
			          device.index(),
			          device.name(),
			          device.api_version(),
			          device.driver_version());
		}
		
		let physical = PhysicalDevice::enumerate(&instance).skip(device.unwrap_or(0)).next().ok_or(RendererError::NoDevices)?;
		
		dprintln!("\nUsing {}: {} api: {} driver: {}",
		          physical.index(),
		          physical.name(),
		          physical.api_version(),
		          physical.driver_version());
		
		Ok(physical)
	}
	
	fn create_device(physical: PhysicalDevice) -> Result<(Arc<Device>, Arc<Queue>, Arc<Queue>), RendererError> {
		for family in physical.queue_families() {
			dprintln!("Found a queue family with {:?} queue(s){}{}{}{}",
		          family.queues_count(),
		          family.supports_graphics().then_some(", Graphics").unwrap_or_default(),
		          family.supports_compute().then_some(", Compute").unwrap_or_default(),
		          family.supports_sparse_binding().then_some(", Sparse").unwrap_or_default(),
		          family.explicitly_supports_transfers().then_some(", Transfers").unwrap_or_default());
		}
		
		let queue_family = physical.queue_families()
		                           .find(|&q| q.supports_graphics())
		                           .ok_or(RendererError::NoQueue)?;
		
		// let load_queue_family = physical.queue_families()
		//                                 .find(|&q| q.explicitly_supports_transfers() && !(q.id() == queue_family.id() && q.queues_count() <= 1))
		//                                 .unwrap_or(queue_family);
		
		let families = vec![
			(queue_family, 0.5),
			// (load_queue_family, 0.2),
		];
		
		let (device, mut queues) = Device::new(physical,
		                                       &Features::none(),
		                                       &DeviceExtensions { khr_swapchain: true,
		                                                           ..DeviceExtensions::none() },
		                                       families.into_iter())?;
		
		let queue = queues.next().ok_or(RendererError::NoQueue)?;
		
		// let load_queue = queues.next().ok_or(RendererCreationError::NoQueue)?;
		// TODO: Get better GPU
		let load_queue = queue.clone();
		
		Ok((device, queue, load_queue))
	}
	
	fn create_render_pass(device: &Arc<Device>) -> Result<Arc<RenderPass>, RendererError> {
		Ok(Arc::new(
			vulkano::single_pass_renderpass!(device.clone(),
				attachments: {
					color: {
						load: Clear,
						store: Store,
						format: eye::IMAGE_FORMAT,
						samples: 1,
					},
					depth: {
						load: Clear,
						store: DontCare,
						format: eye::DEPTH_FORMAT,
						samples: 1,
					}
				},
				pass: {
					color: [color],
					depth_stencil: {depth}
				}
			)?
		))
	}
	
	pub fn create_swapchain<W>(&self, surface: Arc<Surface<W>>) -> Result<(Arc<Swapchain<W>>, Vec<Arc<SwapchainImage<W>>>), RendererSwapchainError> {
		let caps = surface.capabilities(self.device.physical_device())?;
		let dimensions = caps.current_extent.unwrap_or(caps.min_image_extent);
		let format = caps.supported_formats
		                 .iter()
		                 .find(|format| format.0 == Format::B8G8R8A8Unorm || format.0 == Format::R8G8B8A8Unorm)
		                 .expect("UNorm format not supported on the surface");
		
		let alpha_preference = [CompositeAlpha::PreMultiplied, CompositeAlpha::Opaque, CompositeAlpha::Inherit];
		let alpha = alpha_preference.iter()
		                            .cloned()
		                            .find(|&composite| caps.supported_composite_alpha.supports(composite))
		                            .expect("PreMultiplied and Opaque alpha composites not supported on the surface");
		
		let usage = ImageUsage{
			transfer_destination: true,
			sampled: true,
			..ImageUsage::none()
		};
		
		Ok(Swapchain::new(self.device.clone(),
		                  surface,
		                  2.max(caps.min_image_count).min(caps.max_image_count.unwrap_or(2)),
		                  format.0,
		                  dimensions,
		                  1,
		                  usage,
		                  &self.queue,
		                  SurfaceTransform::Identity,
		                  alpha,
		                  PresentMode::Fifo,
		                  FullscreenExclusive::Allowed,
		                  false,
		                  format.1)?)
	}
	
	pub fn render(&mut self, hmd_pose: Isometry3, scene: &mut [Entity], window: &mut Window) -> Result<(), RendererRenderError> {
		self.previous_frame_end.as_mut().unwrap().cleanup_finished();
		
		if window.swapchain_regen_required {
			match window.regen_swapchain() {
				Err(window::WindowSwapchainRegenError::NeedRetry) => {},
				Err(err) => return Err(err.into()),
				Ok(_) => {}
			}
		}
		
		let view_base = hmd_pose.inverse();
		let view_left = &self.eyes.left.view * &view_base;
		let view_right = &self.eyes.right.view * &view_base;
		let light_source = Vec3::new(0.5, -0.5, -1.5).normalize();
		
		let commons = CommonsUBO {
			projection: [self.eyes.left.projection.clone(), self.eyes.right.projection.clone()],
			view: [view_left, view_right],
			light_direction: [
				(view_left * light_source).to_homogeneous(),
				(view_right * light_source).to_homogeneous(),
			],
			ambient: 0.25,
		};
		
		let mut builder = AutoCommandBufferBuilder::new(self.device.clone(), self.queue.family())?;
		builder.update_buffer(self.commons.clone(),
		                      commons.clone())?;
		
		for entity in scene.iter_mut() {
			entity.pre_render(&mut builder)?;
		}
		
		builder.begin_render_pass(self.eyes.left.frame_buffer.clone(),
		                          SubpassContents::Inline,
		                          vec![ ClearValue::Float([0.0, 0.0, 0.0, 0.0]),
		                                ClearValue::Depth(1.0) ])?;
		
		for entity in scene.iter_mut() {
			entity.render(&mut builder, 0)?;
		}
		
		builder.end_render_pass()?;
		
		for entity in scene.iter_mut() {
			entity.pre_render(&mut builder)?;
		}
		
		builder.begin_render_pass(self.eyes.right.frame_buffer.clone(),
		                          SubpassContents::Inline,
		                          vec![ ClearValue::Float([0.0, 0.0, 0.0, 0.0]),
		                                ClearValue::Depth(1.0) ])?;
		
		for entity in scene.iter_mut() {
			entity.render(&mut builder, 1)?;
		}
		
		builder.end_render_pass()?;
		
		let command_buffer = builder.build()?;
		
		let mut future = self.previous_frame_end.take().unwrap();
		
		if !future.queue_change_allowed() && !future.queue().unwrap().is_same(&self.queue) {
			future = Box::new(future.then_signal_semaphore());
		}
		
		future = Box::new(future.then_execute(self.queue.clone(), command_buffer)?);
		
		future = match window.render(&self.device, &self.queue, future, &mut self.eyes.left.image, &mut self.eyes.right.image) {
			Ok(future) => future,
			Err(WindowRenderError::Later(future)) => future,
			Err(err) => return Err(err.into()),
		};
		
		let future = future.then_signal_fence_and_flush();
		
		match future {
			Ok(future) => {
				self.previous_frame_end = Some(Box::new(future) as Box<_>);
			},
			Err(sync::FlushError::OutOfDate) => {
				eprintln!("Flush Error: Out of date, ignoring");
				self.previous_frame_end = Some(Box::new(sync::now(self.device.clone())) as Box<_>);
			},
			Err(err) => return Err(err.into()),
		}
		
		Ok(())
	}
}


#[derive(Debug, Error)]
pub enum RendererError {
	#[error(display = "No devices available.")] NoDevices,
	#[error(display = "No compute queue available.")] NoQueue,
	#[error(display = "{}", _0)] EyeCreationError(#[error(source)] EyeCreationError),
	#[error(display = "{}", _0)] LayersListError(#[error(source)] instance::LayersListError),
	#[error(display = "{}", _0)] InstanceCreationError(#[error(source)] instance::InstanceCreationError),
	#[error(display = "{}", _0)] DeviceCreationError(#[error(source)] device::DeviceCreationError),
	#[error(display = "{}", _0)] OomError(#[error(source)] vulkano::OomError),
	#[error(display = "{}", _0)] RenderPassCreationError(#[error(source)] framebuffer::RenderPassCreationError),
	#[error(display = "{}", _0)] GraphicsPipelineCreationError(#[error(source)] pipeline::GraphicsPipelineCreationError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
}

#[derive(Debug, Error)]
pub enum RendererSwapchainError {
	#[error(display = "{}", _0)] CapabilitiesError(#[error(source)] swapchain::CapabilitiesError),
	#[error(display = "{}", _0)] SwapchainCreationError(#[error(source)] swapchain::SwapchainCreationError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
}

#[derive(Debug, Error)]
pub enum RendererRenderError {
	#[error(display = "{}", _0)] SwapchainRegenError(#[error(source)] WindowSwapchainRegenError),
	#[error(display = "{}", _0)] WindowRenderError(#[error(source)] WindowRenderError),
	#[error(display = "{}", _0)] ModelRenderError(#[error(source)] ModelRenderError),
	#[error(display = "{}", _0)] OomError(#[error(source)] vulkano::OomError),
	#[error(display = "{}", _0)] BeginRenderPassError(#[error(source)] command_buffer::BeginRenderPassError),
	#[error(display = "{}", _0)] DrawIndexedError(#[error(source)] command_buffer::DrawIndexedError),
	#[error(display = "{}", _0)] AutoCommandBufferBuilderContextError(#[error(source)] command_buffer::AutoCommandBufferBuilderContextError),
	#[error(display = "{}", _0)] CommandBufferBuildError(#[error(source)] command_buffer::BuildError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] command_buffer::CommandBufferExecError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] BlitImageError(#[error(source)] command_buffer::BlitImageError),
	#[error(display = "{}", _0)] UpdateBufferError(#[error(source)] command_buffer::UpdateBufferError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
	#[error(display = "{}", _0)] PersistentDescriptorSetError(#[error(source)] descriptor_set::PersistentDescriptorSetError),
	#[error(display = "{}", _0)] PersistentDescriptorSetBuildError(#[error(source)] descriptor_set::PersistentDescriptorSetBuildError),
}
