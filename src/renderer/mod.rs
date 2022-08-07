use std::cell::RefMut;
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::ffi::CString;
use std::sync::{Arc, mpsc};
use err_derive::Error;
use bytemuck::{Pod, Zeroable};
use vulkano::{command_buffer, device, instance, memory, pipeline, render_pass, swapchain, sync, Version};
use vulkano::buffer::{BufferUsage, DeviceLocalBuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, SubpassContents};
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Features, physical, Queue, QueueCreateInfo};
use vulkano::device::physical::PhysicalDevice;
use vulkano::format::Format;
use vulkano::image::{ImageLayout, ImageUsage, SampleCount, SwapchainImage};
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::instance::debug::{DebugCallback, MessageSeverity, MessageType};
use vulkano::render_pass::{AttachmentDescription, AttachmentReference, LoadOp, RenderPass, RenderPassCreateInfo, StoreOp, SubpassDescription};
use vulkano::swapchain::{CompositeAlpha, PresentMode, Surface, SurfaceInfo, Swapchain, SwapchainCreateInfo};
use vulkano::sync::{FenceSignalFuture, GpuFuture};

pub mod camera;
pub mod eyes;
pub mod window;
pub mod pipelines;
pub mod debug_renderer;
pub mod assets_manager;
mod openvr_cb;
mod background;

use crate::{config, debug};
use crate::application::{Entity, VR};
use crate::component::{ComponentError, RenderType};
use crate::math::{AMat4, Color, Isometry3, PMat4, Vec4};
use crate::utils::*;
use background::{Background, BackgroundError, BackgroundRenderError};
use camera::{Camera, CameraStartError};
use debug_renderer::{DebugRenderer, DebugRendererError, DebugRendererRenderError, TextCache, DebugRendererPreRenderError};
use eyes::{EyeCreationError, Eyes};
use openvr_cb::OpenVRCommandBuffer;
use pipelines::Pipelines;
use window::{Window, WindowRenderError, WindowSwapchainRegenError};
use assets_manager::{AssetsManager, AssetKey};


#[allow(dead_code)]
#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct CommonsUBO {
	projection: [NgPod<PMat4>; 2],
	view: [NgPod<AMat4>; 2],
	light_direction: [NgPod<Vec4>; 2],
	ambient: f32,
}

pub struct Renderer {
	pub instance: Arc<Instance>,
	pub device: Arc<Device>,
	pub load_queue: Arc<Queue>,
	pub queue: Arc<Queue>,
	pub pipelines: Pipelines,
	pub commons: Arc<DeviceLocalBuffer<CommonsUBO>>,
	
	vr: Option<Arc<VR>>,
	eyes: Eyes,
	previous_frame_end: Option<FenceSignalFuture<Box<dyn GpuFuture>>>,
	background: Background,
	load_commands: mpsc::Receiver<(PrimaryAutoCommandBuffer, Option<Isometry3>)>,
	debug_renderer: Option<DebugRenderer>,
	fps_counter: FpsCounter<20>,
	assets_manager: Option<AssetsManager>,
	transparent_registry: Vec<(f32, u64)>,
	#[allow(dead_code)] debug_callback: Option<DebugCallback>,
}

impl Renderer {
	pub fn new<C>(vr: Option<Arc<VR>>, camera: C)
	             -> Result<Renderer, RendererError>
	             where C: Camera {
		let instance = Renderer::create_vulkan_instance(&vr)?;
		let debug_callback = config::get()
		                            .validation
		                            .then(|| Renderer::create_debug_callbacks(&instance))
		                            .transpose()?;
		
		let physical = Renderer::create_physical_device(&instance, &vr)?;
		let (device, queue, load_queue) = Renderer::create_device(physical, &vr)?;
		let render_pass = Renderer::create_render_pass(&device)?;
		
		let eyes = if let Some(ref vr) = vr {
			Eyes::new_vr(vr, &queue, &render_pass)?
		} else {
			Eyes::new_novr(&config::get().novr, &queue, &render_pass)?
		};
		
		let commons = DeviceLocalBuffer::new(device.clone(),
		                                     BufferUsage{ transfer_destination: true,
		                                                  uniform_buffer: true,
		                                                  ..BufferUsage::none() },
		                                     Some(queue.family()))?;
		
		let mut pipelines = Pipelines::new(render_pass, eyes.frame_buffer_size);
		let (camera_image, load_commands) = camera.start(load_queue.clone())?;
		let background = Background::new(camera_image, &eyes, &queue, &mut pipelines)?;
		let debug_renderer = Some(DebugRenderer::new(&load_queue, &mut pipelines)?);
		let assets_manager = Some(AssetsManager::new());
		let fps_counter = FpsCounter::new();
		
		Ok(Renderer {
			instance,
			commons,
			vr,
			debug_callback,
			device,
			queue,
			load_queue,
			pipelines,
			eyes,
			previous_frame_end: None,
			background,
			load_commands,
			debug_renderer,
			fps_counter,
			transparent_registry: vec![],
			assets_manager
		})
	}
	
	fn create_vulkan_instance(vr: &Option<Arc<VR>>) -> Result<Arc<Instance>, RendererError> {
		dprintln!("List of Vulkan layers available to use:");
		let available_layers: Vec<_> = vulkano::instance::layers_list()?.collect();
		for layer in &available_layers {
			dprintln!("\t{}", layer.name());
		}
		
		let vr_extensions = vr.as_ref().map(|vr| vr.lock().unwrap().compositor.vulkan_instance_extensions_required()).unwrap_or_default();
		
		let extensions = InstanceExtensions::from(vr_extensions.iter().map(CString::as_c_str))
		                                    .union(&vulkano_win::required_extensions())
		                                    .union(&InstanceExtensions {
			                                    ext_debug_utils: debug::debug(),
			                                    ext_debug_report: debug::debug(), // required by RenderDoc
			                                    khr_get_physical_device_properties2: true, // required by multiview
			                                    khr_external_semaphore_capabilities: true, // required by khr_external_semaphore from vr_extensions
			                                    ..InstanceExtensions::none()
		                                    });
		
		let mut layers = vec![];
		
		if config::get().validation {
			layers.push("VK_LAYER_KHRONOS_validation".to_string());
		}
		
		let removed = layers.drain_filter(|layer| available_layers.iter().all(|al| al.name() != layer));
		
		for layer in removed {
			eprintln!("MISSING LAYER: {}", layer);
		}
		
		Ok(Instance::new(InstanceCreateInfo {
			engine_version: Version::V1_2,
			enabled_extensions: extensions,
			enabled_layers: layers,
			..InstanceCreateInfo::application_from_cargo_toml()
		})?)
	}
	
	fn create_debug_callbacks(instance: &Arc<Instance>) -> Result<DebugCallback, RendererError> {
		let severity = MessageSeverity {
			error:       true,
			warning:     true,
			information: true,
			verbose:     true,
		};
		
		let ty = MessageType::all();
		
		Ok(DebugCallback::new(instance, severity, ty, |msg| {
			if !debug::debug() { return }
			if msg.ty.general && msg.severity.verbose { return }
			
			// debug::debugger();
			
			let severity = if msg.severity.error {
				"error"
			} else if msg.severity.warning {
				"warning"
			} else if msg.severity.information {
				"information"
			} else if msg.severity.verbose {
				"verbose"
			} else {
				"unknown"
			};
			
			let ty = if msg.ty.general {
				"general"
			} else if msg.ty.validation {
				"validation"
			} else if msg.ty.performance {
				"performance"
			} else {
				"unknown"
			};
			
			println!("{} {} {}: {}",
			         msg.layer_prefix.unwrap_or("UNKNOWN"),
			         ty,
			         severity,
			         msg.description);
		})?)
	}
	
	fn create_physical_device<'a>(instance: &'a Arc<Instance>, vr: &Option<Arc<VR>>) -> Result<PhysicalDevice<'a>, RendererError> {
		dprintln!("Devices:");
		for device in PhysicalDevice::enumerate(&instance) {
			dprintln!("\t{}: {} api: {} driver: {}",
			          device.index(),
			          device.properties().device_name,
			          device.properties().api_version,
			          device.properties().driver_version);
		}
		
		let physical = vr.as_ref()
		                 .and_then(|vr| vr.lock().unwrap().system.vulkan_output_device(instance.as_ptr()))
		                 .and_then(|ptr| PhysicalDevice::enumerate(&instance).find(|physical| physical.as_ptr() == ptr))
		                 .or_else(|| {
			                 if vr.is_some() { println!("Failed to fetch device from openvr, using fallback"); }
			                 PhysicalDevice::enumerate(&instance).skip(config::get().gpu_id).next()
		                 })
		                 .ok_or(RendererError::NoDevices)?;
		
		if physical.properties().max_multiview_view_count.unwrap_or(0) < 2 {
			return Err(RendererError::MultiviewNotSupported);
		}
		
		dprintln!("\nUsing {}: {} api: {} driver: {}",
		          physical.index(),
		          physical.properties().device_name,
		          physical.properties().api_version,
		          physical.properties().driver_version);
		
		Ok(physical)
	}
	
	fn create_device(physical: PhysicalDevice, vr: &Option<Arc<VR>>) -> Result<(Arc<Device>, Arc<Queue>, Arc<Queue>), RendererError> {
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
		
		// TODO: q.supports_graphics() prevents you from using pure transfer-oriented families, but it's required for mipmaps. Something has to be done about it.
		let load_queue_family = physical.queue_families()
		                                .find(|&q| q.explicitly_supports_transfers() && q.supports_graphics() && q.id() != queue_family.id());
		
		let mut queue_create_infos = vec![QueueCreateInfo::family(queue_family)];
		
		if let Some(load_queue_family) = load_queue_family {
			queue_create_infos.push(QueueCreateInfo::family(load_queue_family));
		}
		
		let vr_extensions = vr.as_ref().map(|vr| vulkan_device_extensions_required(&vr.lock().unwrap().compositor, &physical)).unwrap_or_default();
		
		let (device, mut queues) = Device::new(physical, DeviceCreateInfo {
			enabled_extensions: DeviceExtensions::from(vr_extensions.iter().map(CString::as_c_str)).union(&DeviceExtensions {
				khr_swapchain: true,
				khr_storage_buffer_storage_class: true,
				..DeviceExtensions::none()
			}),
			enabled_features: Features {
				multiview: true,
				..Features::none()
			},
			queue_create_infos,
			..DeviceCreateInfo::default()
		})?;
		
		let queue = queues.next().ok_or(RendererError::NoQueue)?;
		let load_queue = queues.next().unwrap_or_else(|| queue.clone());
		
		Ok((device, queue, load_queue))
	}
	
	fn create_render_pass(device: &Arc<Device>) -> Result<Arc<RenderPass>, RendererError> {
		let msaa = config::get().msaa;
		let samples = msaa.try_into().map_err(|_| RendererError::InvalidMultiSamplingCount(msaa))?;
		
		let mut attachments = vec![
			AttachmentDescription {
				format: Some(eyes::IMAGE_FORMAT),
				samples,
				load_op: LoadOp::DontCare,
				store_op: StoreOp::Store,
				initial_layout: ImageLayout::ColorAttachmentOptimal,
				final_layout: ImageLayout::ColorAttachmentOptimal,
				..AttachmentDescription::default()
			},
			AttachmentDescription {
				format: Some(eyes::DEPTH_FORMAT),
				samples,
				load_op: LoadOp::Clear,
				store_op: StoreOp::DontCare,
				initial_layout: ImageLayout::DepthStencilAttachmentOptimal,
				final_layout: ImageLayout::DepthStencilAttachmentOptimal,
				..AttachmentDescription::default()
			},
		];
		
		let mut subpasses = vec![
			SubpassDescription {
				view_mask: 0b11,
				color_attachments: vec![Some(AttachmentReference {
					attachment: 0,
					layout: attachments[0].final_layout,
					..AttachmentReference::default()
				})],
				depth_stencil_attachment: Some(AttachmentReference {
					attachment: 1,
					layout: attachments[1].final_layout,
					..AttachmentReference::default()
				}),
				input_attachments: vec![],
				resolve_attachments: vec![],
				preserve_attachments: vec![],
				..SubpassDescription::default()
			},
		];
		
		if samples != SampleCount::Sample1 {
			attachments.push(AttachmentDescription {
				format: Some(eyes::IMAGE_FORMAT),
				samples: SampleCount::Sample1,
				load_op: LoadOp::DontCare,
				store_op: StoreOp::Store,
				initial_layout: ImageLayout::TransferDstOptimal,
				final_layout: ImageLayout::TransferDstOptimal,
				..AttachmentDescription::default()
			});
			
			subpasses[0].resolve_attachments.push(Some(AttachmentReference {
				attachment: 2,
				layout: attachments[2].final_layout,
				..AttachmentReference::default()
			}))
		}
		
		let render_pass = RenderPass::new(device.clone(), RenderPassCreateInfo {
			attachments,
			subpasses,
			dependencies: vec![],
			correlated_view_masks: vec![0b11],
			..RenderPassCreateInfo::default()
		})?;
		
		Ok(render_pass)
	}
	
	pub fn create_swapchain<W>(&self, surface: Arc<Surface<W>>) -> Result<(Arc<Swapchain<W>>, Vec<Arc<SwapchainImage<W>>>), RendererSwapchainError> {
		if !self.queue.family().supports_surface(&surface)? {
			return Err(RendererSwapchainError::SurfaceNotSupported)
		}
		
		let caps = self.device
		               .physical_device()
		               .surface_capabilities(&surface, SurfaceInfo::default())?;
		
		let dimensions = caps.current_extent.unwrap_or(caps.min_image_extent);
		let format = self.device
		                 .physical_device()
		                 .surface_formats(&surface, SurfaceInfo::default())?
		                 .iter()
		                 .find(|format| format.0 == Format::B8G8R8A8_UNORM || format.0 == Format::R8G8B8A8_UNORM)
		                 .expect("UNorm format not supported on the surface")
		                 .clone();
		
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
		
		Ok(Swapchain::new(self.device.clone(), surface, SwapchainCreateInfo {
			min_image_count: 2.max(caps.min_image_count).min(caps.max_image_count.unwrap_or(caps.min_image_count)),
			image_format: Some(format.0),
			image_color_space: format.1,
			image_extent: dimensions,
			image_usage: usage,
			composite_alpha: alpha,
			present_mode: PresentMode::Fifo,
			clipped: false,
			..SwapchainCreateInfo::default()
		})?)
	}
	
	pub fn render(&mut self, hmd_pose: [[f32; 4]; 3], camera_pos: Isometry3, scene: &mut BTreeMap<u64, Entity>, window: &mut Window) -> Result<(), RendererRenderError> {
		if window.swapchain_regen_required {
			match window.regen_swapchain() {
				Err(window::WindowSwapchainRegenError::NeedRetry) => {},
				Err(err) => return Err(err.into()),
				Ok(_) => {}
			}
		}
		
		let mut future = if let Some(mut previous_frame_end) = self.previous_frame_end.take() {
			previous_frame_end.cleanup_finished();
			previous_frame_end.wait(None)?;
			previous_frame_end.boxed()
		} else {
			sync::now(self.device.clone()).boxed()
		};
		
		// TODO: Optimize Boxes
		while let Ok((command, cam_pose)) = self.load_commands.try_recv() {
			if !future.queue_change_allowed() && &future.queue().unwrap() != &self.load_queue {
				future = Box::new(future.then_signal_semaphore()
				                        .then_execute(self.load_queue.clone(), command)?);
			} else {
				future = Box::new(future.then_execute(self.load_queue.clone(), command)?);
			}
			
			self.background.update_frame_pose(cam_pose.unwrap_or(camera_pos));
		}
		
		self.fps_counter.tick();
		
		debug::draw_text(format!("FPS: {}", self.fps_counter.fps().floor()), point!(-1.0, -1.0), debug::DebugOffset::bottom_right(16.0, 16.0), 64.0, Color::green());
		debug::draw_text(format!("CAM FPS: {}", debug::get_flag_or_default::<f32>("CAMERA_FPS").floor()), point!(-1.0, -1.0), debug::DebugOffset::bottom_right(16.0, 96.0), 64.0, Color::green());
		
		let view_base = camera_pos.inverse();
		let view_left = &self.eyes.view.0 * &view_base;
		let view_right = &self.eyes.view.1 * &view_base;
		let light_source = vector!(0.5, -0.5, -1.5).normalize();
		let pixel_scale = vector!(1.0 / self.eyes.frame_buffer_size.0 as f32, 1.0 / self.eyes.frame_buffer_size.1 as f32) * config::get().ssaa;
		
		let commons = CommonsUBO {
			projection: [self.eyes.projection.0.clone().into(), self.eyes.projection.1.clone().into()],
			view: [view_left.into(), view_right.into()],
			light_direction: [
				(view_left * light_source).to_homogeneous().into(),
				(view_right * light_source).to_homogeneous().into(),
			],
			ambient: 0.25,
		};
		
		let (eye_width, eye_height) = self.eyes.frame_buffer_size;
		
		let mut builder = AutoCommandBufferBuilder::primary(self.device.clone(), self.queue.family(), CommandBufferUsage::OneTimeSubmit)?;
		builder.update_buffer(self.commons.clone(),
		                      Arc::new(commons.clone()))?;
		
		for entity in scene.values_mut() {
			let transparent = entity.pre_render(&self, &mut builder)?;
			
			if transparent {
				self.transparent_registry.push(((entity.state().position.translation.vector - camera_pos.translation.vector).magnitude(), entity.id));
			}
		}
		
		{
			let mut debug_renderer = self.debug_renderer.take().unwrap();
			debug_renderer.pre_render(self)?;
			self.debug_renderer = Some(debug_renderer);
		}
		
		builder.begin_render_pass(self.eyes.frame_buffer.clone(),
		                          SubpassContents::Inline,
		                          self.eyes.clear_values.iter().copied())?;
		
		self.background.render(&mut builder, camera_pos)?;
		
		for entity in scene.values_mut() {
			entity.render(&self, RenderType::Opaque, &mut builder)?;
		}
		
		self.transparent_registry.sort_unstable_by(|(a, _), (b, _)| b.partial_cmp(a).unwrap());
		
		for (_, entity) in self.transparent_registry.iter() {
			scene.get_mut(&entity).unwrap().render(&self, RenderType::Transparent, &mut builder)?;
		}
		
		self.transparent_registry.clear();
		
		self.debug_renderer.as_mut().unwrap().render(&mut builder, &commons, pixel_scale)?;
		
		builder.end_render_pass()?
		       .copy_image(self.eyes.resolved_image.clone(),
		                   [0, 0, 0],
		                   1,
		                   0,
		                   self.eyes.side_image.clone(),
		                   [0, 0, 0],
		                   0,
		                   0,
		                   [eye_width as u32, eye_height as u32, 1],
		                   1)?;
		
		let command_buffer = builder.build()?;
		
		if !future.queue_change_allowed() && &future.queue().unwrap() != &self.queue {
			future = future.then_signal_semaphore().boxed();
		}
		
		future = future.then_execute(self.queue.clone(), command_buffer)?.boxed();
		
		// TODO: Explicit timing mode
		if let Some(ref vr) = self.vr {
			let vr = vr.lock().unwrap();
			
			// Safety: OpenVRCommandBuffer::end must be executed(flused) after start to not leave eye textures in an unexpected layout
			unsafe {
				let f = future.then_execute(self.queue.clone(), OpenVRCommandBuffer::start(self.eyes.resolved_image.clone(), self.device.clone(), self.queue.family())?)?
				              .then_execute(self.queue.clone(), OpenVRCommandBuffer::start(self.eyes.side_image.clone(), self.device.clone(), self.queue.family())?)?;
				f.flush()?;
				
				let debug = debug::debug();
				if debug { debug::set_debug(false); } // Hide internal OpenVR warnings (https://github.com/ValveSoftware/openvr/issues/818)
				vr.compositor.submit(openvr::Eye::Left,  &self.eyes.textures.0, None, Some(hmd_pose))?;
				vr.compositor.submit(openvr::Eye::Right, &self.eyes.textures.1, None, Some(hmd_pose))?;
				if debug { debug::set_debug(true); }
				
				future = f.then_execute(self.queue.clone(), OpenVRCommandBuffer::end(self.eyes.resolved_image.clone(),  self.device.clone(), self.queue.family())?)?
				          .then_execute(self.queue.clone(), OpenVRCommandBuffer::end(self.eyes.side_image.clone(),  self.device.clone(), self.queue.family())?)?
				          .boxed();
			}
		}
		
		future = match window.render(&self.device, &self.queue, future, &self.eyes.resolved_image) {
			Ok(future) => future,
			Err(WindowRenderError::Later(future)) => future,
			Err(err) => return Err(err.into()),
		};
		
		self.debug_renderer.as_mut().unwrap().text_cache.borrow_mut().cleanup();
		
		match future.then_signal_fence_and_flush() {
			Ok(future) => {
				self.previous_frame_end = Some(future);
			},
			Err(sync::FlushError::OutOfDate) => {
				eprintln!("Flush Error: Out of date, ignoring");
			},
			Err(err) => return Err(err.into()),
		}
		
		Ok(())
	}
	
	pub fn debug_text_cache(&self) -> RefMut<TextCache> {
		self.debug_renderer.as_ref().unwrap().text_cache.borrow_mut()
	}
	
	pub fn load<Key: AssetKey + 'static>(&mut self, key: Key) -> Result<Key::Asset, Key::Error> {
		let mut assets_manager = self.assets_manager.take().unwrap();
		let result = assets_manager.load(key, self);
		self.assets_manager = Some(assets_manager);
		result
	}
}


#[derive(Debug, Error)]
pub enum RendererError {
	#[error(display = "No devices available.")] NoDevices,
	#[error(display = "No compute queue available.")] NoQueue,
	#[error(display = "Multiview doesn't support enough views.")] MultiviewNotSupported,
	#[error(display = "Invalid Multi-Sampling count: {}", _0)] InvalidMultiSamplingCount(u32),
	#[error(display = "{}", _0)] EyeCreationError(#[error(source)] EyeCreationError),
	#[error(display = "{}", _0)] CameraStartError(#[error(source)] CameraStartError),
	#[error(display = "{}", _0)] DebugRendererError(#[error(source)] DebugRendererError),
	#[error(display = "{}", _0)] BackgroundError(#[error(source)] BackgroundError),
	#[error(display = "{}", _0)] LayersListError(#[error(source)] instance::LayersListError),
	#[error(display = "{}", _0)] InstanceCreationError(#[error(source)] instance::InstanceCreationError),
	#[error(display = "{}", _0)] DebugCallbackCreationError(#[error(source)] instance::debug::DebugCallbackCreationError),
	#[error(display = "{}", _0)] DeviceCreationError(#[error(source)] device::DeviceCreationError),
	#[error(display = "{}", _0)] OomError(#[error(source)] vulkano::OomError),
	#[error(display = "{}", _0)] RenderPassCreationError(#[error(source)] render_pass::RenderPassCreationError),
	#[error(display = "{}", _0)] GraphicsPipelineCreationError(#[error(source)] pipeline::graphics::GraphicsPipelineCreationError),
	#[error(display = "{}", _0)] DeviceMemoryAllocationError(#[error(source)] memory::DeviceMemoryAllocationError),
}

#[derive(Debug, Error)]
pub enum RendererSwapchainError {
	#[error(display = "Surface presentation is not supported.")] SurfaceNotSupported,
	#[error(display = "{}", _0)] SurfacePropertiesError(#[error(source)] physical::SurfacePropertiesError),
	#[error(display = "{}", _0)] SwapchainCreationError(#[error(source)] swapchain::SwapchainCreationError),
}

#[derive(Debug, Error)]
pub enum RendererRenderError {
	#[error(display = "{}", _0)] SwapchainRegenError(#[error(source)] WindowSwapchainRegenError),
	#[error(display = "{}", _0)] WindowRenderError(#[error(source)] WindowRenderError),
	#[error(display = "{}", _0)] DebugRendererPreRenderError(#[error(source)] DebugRendererPreRenderError),
	#[error(display = "{}", _0)] DebugRendererRenderError(#[error(source)] DebugRendererRenderError),
	#[error(display = "{}", _0)] BackgroundRenderError(#[error(source)] BackgroundRenderError),
	#[error(display = "{}", _0)] ComponentError(ComponentError),
	#[error(display = "{}", _0)] OomError(#[error(source)] vulkano::OomError),
	#[error(display = "{}", _0)] BeginRenderPassError(#[error(source)] command_buffer::BeginRenderPassError),
	#[error(display = "{}", _0)] DrawIndexedError(#[error(source)] command_buffer::DrawIndexedError),
	#[error(display = "{}", _0)] AutoCommandBufferBuilderContextError(#[error(source)] command_buffer::AutoCommandBufferBuilderContextError),
	#[error(display = "{}", _0)] CommandBufferBuildError(#[error(source)] command_buffer::BuildError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] command_buffer::CommandBufferExecError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] BlitImageError(#[error(source)] command_buffer::BlitImageError),
	#[error(display = "{}", _0)] CopyImageError(#[error(source)] command_buffer::CopyImageError),
	#[error(display = "{}", _0)] UpdateBufferError(#[error(source)] command_buffer::UpdateBufferError),
	#[error(display = "{}", _0)] CompositorError(#[error(source)] openvr::compositor::CompositorError),
}

impl From<ComponentError> for RendererRenderError {
	fn from(err: ComponentError) -> Self {
		RendererRenderError::ComponentError(err)
	}
}
