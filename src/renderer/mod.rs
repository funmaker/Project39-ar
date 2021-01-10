use std::sync::{Arc, mpsc};
use err_derive::Error;
use vulkano::{app_info_from_cargo_toml, OomError, format};
use vulkano::device::{Device, DeviceExtensions, RawDeviceExtensions, Features, Queue, DeviceCreationError};
use vulkano::instance::debug::{DebugCallback, MessageSeverity, MessageType};
use vulkano::instance::{Instance, InstanceExtensions, RawInstanceExtensions, PhysicalDevice, LayersListError, InstanceCreationError};
use vulkano::pipeline::GraphicsPipelineCreationError;
use vulkano::sync::{GpuFuture, FlushError};
use vulkano::sync;
use vulkano::framebuffer::{RenderPassCreationError, RenderPassAbstract};
use vulkano::command_buffer::{AutoCommandBufferBuilder, BeginRenderPassError, AutoCommandBufferBuilderContextError, BuildError, CommandBufferExecError, DrawIndexedError, BlitImageError, AutoCommandBuffer, UpdateBufferError, SubpassContents};
use vulkano::swapchain::{Swapchain, SurfaceTransform, PresentMode, FullscreenExclusive, Surface, CapabilitiesError, SwapchainCreationError, CompositeAlpha};
use vulkano::memory::DeviceMemoryAllocError;
use vulkano::format::{ClearValue, Format};
use vulkano::image::{AttachmentImage, SwapchainImage};
use vulkano::sampler::Filter;
use vulkano::buffer::{BufferUsage, DeviceLocalBuffer};
use openvr::{System, Compositor};
use cgmath::{Matrix4, Transform, Matrix, Vector3, Vector4, InnerSpace, Rad, Deg, Matrix3};
use openvr::compositor::CompositorError;

pub mod model;
pub mod camera;
pub mod eye;
pub mod window;
pub mod pipelines;
pub mod entity;

use crate::utils::*;
use crate::debug::debug;
use crate::application::VR;
use camera::{CameraStartError, Camera};
use eye::{Eye, Eyes, EyeCreationError};
use window::Window;
use pipelines::Pipelines;
use entity::Entity;

type RenderPass = dyn RenderPassAbstract + Send + Sync;

#[allow(dead_code)]
pub struct CommonsVBO {
	projection: [Matrix4<f32>; 2],
	view: [Matrix4<f32>; 2],
	light_direction: [Vector4<f32>; 2],
	ambient: f32,
}

pub struct Renderer {
	pub instance: Arc<Instance>,
	pub commons: Arc<DeviceLocalBuffer<CommonsVBO>>,
	
	vr: Option<Arc<VR>>,
	device: Arc<Device>,
	queue: Arc<Queue>,
	load_queue: Arc<Queue>,
	pipelines: Pipelines,
	eyes: Eyes,
	previous_frame_end: Option<Box<dyn GpuFuture>>,
	camera_image: Arc<AttachmentImage<format::B8G8R8A8Unorm>>,
    load_commands: mpsc::Receiver<AutoCommandBuffer>,
}

impl Renderer {
	pub fn new<C>(vr: Option<Arc<VR>>, device: Option<usize>, camera: C)
	             -> Result<Renderer, RendererCreationError>
	             where C: Camera {
		let instance = Renderer::create_vulkan_instance(&vr)?;
		
		if debug() {
			Renderer::install_debug_callbacks(&instance);
		}
		
		let physical = Renderer::create_physical_device(device, &instance, &vr)?;
		let (device, queue, load_queue) = Renderer::create_device(physical, &vr)?;
		let render_pass = Renderer::create_render_pass(&device)?;
		
		let eyes = if let Some(ref vr) = vr {
			Eyes::new_vr(vr, &queue, &render_pass)?
		} else {
			Eyes::new(&queue, &render_pass)?
		};
		
		let previous_frame_end = Some(Box::new(sync::now(device.clone())) as Box<_>);
		
		let commons = DeviceLocalBuffer::new(device.clone(),
		                                     BufferUsage{ transfer_destination: true,
		                                                  uniform_buffer: true,
		                                                  ..BufferUsage::none() },
		                                     Some(queue.family()))?;
		
		let (camera_image, load_commands) = camera.start(load_queue.clone())?;
		
		let pipelines = Pipelines::new(render_pass, eyes.frame_buffer_size);
		
		Ok(Renderer {
			vr,
			instance,
			commons,
			device,
			queue,
			load_queue,
			pipelines,
			eyes,
			previous_frame_end,
			camera_image,
			load_commands,
		})
	}
	
	fn create_vulkan_instance(vr: &Option<Arc<VR>>) -> Result<Arc<Instance>, RendererCreationError> {
		dprintln!("List of Vulkan debugging layers available to use:");
		let layers = vulkano::instance::layers_list()?;
		for layer in layers {
			dprintln!("\t{}", layer.name());
		}
		
		let app_infos = app_info_from_cargo_toml!();
		
		let vr_extensions = vr.as_ref().map(|vr| vr.compositor.vulkan_instance_extensions_required()).unwrap_or_default();
		
		let extensions = RawInstanceExtensions::new(vr_extensions)
		                                       .union(&(&vulkano_win::required_extensions()).into())
		                                       .union(&(&InstanceExtensions { ext_debug_utils: debug(),
		                                                                      ..InstanceExtensions::none() }).into());
		
		let layers = if debug() {
			// TODO: Get better GPU
			vec![/*"VK_LAYER_LUNARG_standard_validation"*/]
		} else {
			vec![]
		};
		
		Ok(Instance::new(Some(&app_infos), extensions, layers)?)
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
	
	fn create_physical_device<'a>(device: Option<usize>, instance: &'a Arc<Instance>, vr: &Option<Arc<VR>>) -> Result<PhysicalDevice<'a>, RendererCreationError> {
		dprintln!("Devices:");
		for device in PhysicalDevice::enumerate(&instance) {
			dprintln!("\t{}: {} api: {} driver: {}",
			          device.index(),
			          device.name(),
			          device.api_version(),
			          device.driver_version());
		}
		
		let physical = vr.and_then(|vr| vr.system.vulkan_output_device(instance.as_ptr()))
		                 .and_then(|ptr| PhysicalDevice::enumerate(&instance).find(|physical| physical.as_ptr() == ptr))
		                 .or_else(|| {
			                 if vr.is_some() { println!("Failed to fetch device from openvr, using fallback"); }
			                 PhysicalDevice::enumerate(&instance).skip(device.unwrap_or(0)).next()
		                 })
		                 .ok_or(RendererCreationError::NoDevices)?;
		
		dprintln!("\nUsing {}: {} api: {} driver: {}",
		          physical.index(),
		          physical.name(),
		          physical.api_version(),
		          physical.driver_version());
		
		Ok(physical)
	}
	
	fn create_device(physical: PhysicalDevice, vr: &Option<Arc<VR>>) -> Result<(Arc<Device>, Arc<Queue>, Arc<Queue>), RendererCreationError> {
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
		                           .ok_or(RendererCreationError::NoQueue)?;
		
		// let load_queue_family = physical.queue_families()
		//                                 .find(|&q| q.explicitly_supports_transfers() && !(q.id() == queue_family.id() && q.queues_count() <= 1))
		//                                 .unwrap_or(queue_family);
		
		let families = vec![
			(queue_family, 0.5),
			// (load_queue_family, 0.2),
		];
		
		let vr_extensions = vr.as_ref().map(|vr| vulkan_device_extensions_required(&vr.compositor, &physical)).unwrap_or_default();
		
		let (device, mut queues) = Device::new(physical,
		                                       &Features::none(),
		                                       RawDeviceExtensions::new(vr_extensions)
			                                       .union(&(&DeviceExtensions { khr_swapchain: true,
				                                       ..DeviceExtensions::none() }).into()),
		                                       families.into_iter())?;
		
		let queue = queues.next().ok_or(RendererCreationError::NoQueue)?;
		
		// Mipmaps generation requires graphical queue.
		// TODO: Generate mipmaps on CPU side?
		// let load_queue = queues.next().ok_or(RendererCreationError::NoQueue)?;
		let load_queue = queue.clone();
		
		Ok((device, queue, load_queue))
	}
	
	fn create_render_pass(device: &Arc<Device>) -> Result<Arc<RenderPass>, RendererCreationError> {
		Ok(Arc::new(
			vulkano::single_pass_renderpass!(device.clone(),
				attachments: {
					color: {
						load: Load,
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
		
		Ok(Swapchain::new(self.device.clone(),
		                  surface,
		                  caps.min_image_count,
		                  format.0,
		                  dimensions,
		                  1,
		                  caps.supported_usage_flags,
		                  &self.queue,
		                  SurfaceTransform::Identity,
		                  alpha,
		                  PresentMode::Fifo,
		                  FullscreenExclusive::Allowed,
		                  false,
		                  format.1)?)
	}
	
	pub fn render(&mut self, hmd_pose: Matrix4<f32>, scene: &mut [Entity], window: &mut Window) -> Result<(), RenderError> {
		self.previous_frame_end.as_mut().unwrap().cleanup_finished();
		
		if window.swapchain_regen_required {
			window.regen_swapchain()?;
		}
		
		let view_matrix  = hmd_pose.inverse_transform().unwrap();
		let light_source = Vector3::new(0.5, -0.5, -1.5).normalize();
		
		let [camera_width, camera_height] = self.camera_image.dimensions();
		let (eye_width, eye_height) = self.eyes.frame_buffer_size;
		
		let mut builder = AutoCommandBufferBuilder::new(self.device.clone(), self.queue.family())?;
		builder.blit_image(self.camera_image.clone(),
		                   [0, 0, 0],
		                   [camera_width as i32 / 2, camera_height as i32, 1],
		                   0,
		                   0,
		                   self.eyes.left.image.clone(),
		                   [0, 0, 0],
		                   [eye_width as i32, eye_height as i32, 1],
		                   0,
		                   0,
		                   1,
		                   Filter::Linear)?
		       .blit_image(self.camera_image.clone(),
		                   [camera_width as i32 / 2, 0, 0],
		                   [camera_width as i32, camera_height as i32, 1],
		                   0,
		                   0,
		                   self.eyes.right.image.clone(),
		                   [0, 0, 0],
		                   [eye_width as i32, eye_height as i32, 1],
		                   0,
		                   0,
		                   1,
		                   Filter::Linear)?
		       .update_buffer(self.commons.clone(),
		                      CommonsVBO{
			                      projection: [self.eyes.left.projection, self.eyes.right.projection],
			                      view: [view_matrix, view_matrix],
			                      light_direction: [
				                      (mat3(view_matrix) * light_source).extend(0.0),
				                      (mat3(view_matrix) * light_source).extend(0.0),
			                      ],
			                      ambient: 0.25,
		                      })?
		       .begin_render_pass(self.eyes.left.frame_buffer.clone(),
		                          SubpassContents::Inline,
		                          vec![ ClearValue::None,
		                                ClearValue::Depth(1.0) ])?;
		
		for entity in scene.iter() {
			entity.render(&mut builder, 0)?;
		}
		
		builder.end_render_pass()?
		       .begin_render_pass(self.eyes.right.frame_buffer.clone(),
		                          SubpassContents::Inline,
		                          vec![ ClearValue::None,
		                                ClearValue::Depth(1.0) ])?;
		
		for entity in scene.iter() {
			entity.render(&mut builder, 1)?;
		}
		
		builder.end_render_pass()?;
		
		let command_buffer = builder.build()?;
		
		let mut future = self.previous_frame_end.take().unwrap();
		
		// TODO: Optimize Boxes
		while let Ok(command) = self.load_commands.try_recv() {
			if !future.queue_change_allowed() && !future.queue().unwrap().is_same(&self.load_queue) {
				future = Box::new(future.then_signal_semaphore()
				                           .then_execute(self.load_queue.clone(), command)?);
			} else {
				future = Box::new(future.then_execute(self.load_queue.clone(), command)?);
			}
		}
		
		if !future.queue_change_allowed() && !future.queue().unwrap().is_same(&self.queue) {
			future = Box::new(future.then_signal_semaphore());
		}
		
		if let Some(ref vr) = self.vr {
			unsafe {
				vr.compositor.submit(openvr::Eye::Left,  &self.eyes.left.texture, None, Some(mat34(hmd_pose)))?;
				vr.compositor.submit(openvr::Eye::Right, &self.eyes.right.texture, None, Some(mat34(hmd_pose)))?;
			}
		}
		
		future = Box::new(future.then_execute(self.queue.clone(), command_buffer)?);
		
		if window.render_required {
			future = window.render(&self.device, &self.queue, future, &mut self.eyes.left.image, &mut self.eyes.right.image)?;
		}
		
		let future = future.then_signal_fence_and_flush();
		
		match future {
			Ok(future) => {
				self.previous_frame_end = Some(Box::new(future) as Box<_>);
			},
			Err(FlushError::OutOfDate) => {
				eprintln!("Flush Error: Out of date, ignoring");
				self.previous_frame_end = Some(Box::new(sync::now(self.device.clone())) as Box<_>);
			},
			Err(err) => return Err(err.into()),
		}
		
		Ok(())
	}
}


#[derive(Debug, Error)]
pub enum RendererCreationError {
	#[error(display = "No devices available.")] NoDevices,
	#[error(display = "No compute queue available.")] NoQueue,
	#[error(display = "{}", _0)] LayersListError(#[error(source)] LayersListError),
	#[error(display = "{}", _0)] InstanceCreationError(#[error(source)] InstanceCreationError),
	#[error(display = "{}", _0)] DeviceCreationError(#[error(source)] DeviceCreationError),
	#[error(display = "{}", _0)] OomError(#[error(source)] OomError),
	#[error(display = "{}", _0)] RenderPassCreationError(#[error(source)] RenderPassCreationError),
	#[error(display = "{}", _0)] GraphicsPipelineCreationError(#[error(source)] GraphicsPipelineCreationError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] DeviceMemoryAllocError),
	#[error(display = "{}", _0)] EyeCreationError(#[error(source)] EyeCreationError),
	#[error(display = "{}", _0)] CameraStartError(#[error(source)] CameraStartError),
}

#[derive(Debug, Error)]
pub enum RendererSwapchainError {
	#[error(display = "{}", _0)] CapabilitiesError(#[error(source)] CapabilitiesError),
	#[error(display = "{}", _0)] SwapchainCreationError(#[error(source)] SwapchainCreationError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] DeviceMemoryAllocError),
}

#[derive(Debug, Error)]
pub enum RenderError {
	#[error(display = "Kek")] Kek,
	#[error(display = "{}", _0)] OomError(#[error(source)] OomError),
	#[error(display = "{}", _0)] BeginRenderPassError(#[error(source)] BeginRenderPassError),
	#[error(display = "{}", _0)] DrawIndexedError(#[error(source)] DrawIndexedError),
	#[error(display = "{}", _0)] AutoCommandBufferBuilderContextError(#[error(source)] AutoCommandBufferBuilderContextError),
	#[error(display = "{}", _0)] BuildError(#[error(source)] BuildError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] CommandBufferExecError),
	#[error(display = "{}", _0)] CompositorError(#[error(source)] CompositorError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] FlushError),
	#[error(display = "{}", _0)] BlitImageError(#[error(source)] BlitImageError),
	#[error(display = "{}", _0)] UpdateBufferError(#[error(source)] UpdateBufferError),
	#[error(display = "{}", _0)] SwapchainRegenError(#[error(source)] window::SwapchainRegenError),
	#[error(display = "{}", _0)] WindowRenderError(#[error(source)] window::RenderError),
}
