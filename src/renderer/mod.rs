use std::cell::RefMut;
use std::collections::HashMap;
use std::convert::TryInto;
use std::error::Error;
use std::iter::FromIterator;
use std::sync::Arc;
use colored::Colorize;
use err_derive::Error;
use vulkano::{command_buffer, device, instance, render_pass, swapchain, sync, Version, VulkanLibrary};
use vulkano::buffer::{BufferContents, BufferError, Buffer, Subbuffer, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents};
use vulkano::command_buffer::allocator::{StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Features, physical, Queue, QueueCreateInfo, QueueFlags};
use vulkano::device::physical::PhysicalDevice;
use vulkano::format::{ClearValue, Format};
use vulkano::image::{AttachmentImage, ImageLayout, ImageUsage, ImageViewAbstract, SampleCount, SwapchainImage};
use vulkano::image::view::ImageView;
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::instance::debug::{DebugUtilsMessenger, DebugUtilsMessengerCreateInfo, DebugUtilsMessageType, DebugUtilsMessageSeverity};
use vulkano::memory::allocator::{MemoryUsage, StandardMemoryAllocator};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{AttachmentDescription, AttachmentReference, Framebuffer, FramebufferCreateInfo, LoadOp, RenderPass, RenderPassCreateInfo, StoreOp, SubpassDescription};
use vulkano::swapchain::{CompositeAlpha, PresentMode, Surface, SurfaceInfo, Swapchain, SwapchainCreateInfo};
use vulkano::sync::future::GpuFuture;

pub mod assets_manager;
pub mod context;
pub mod debug_renderer;
pub mod pipelines;
pub mod render_target;

use crate::{config, debug};
use crate::application::{Entity, VR};
use crate::component::ComponentError;
use crate::math::{AMat4, Color, Isometry3, PMat4, Vec4};
use crate::utils::*;
pub use context::{RenderContext, RenderTargetContext, RenderType};
pub use render_target::RenderTarget;
use assets_manager::{AssetKey, AssetsManager};
use debug_renderer::{DebugRenderer, DebugRendererError, DebugRendererPreRenderError, DebugRendererRenderError, TextCache};
use pipelines::Pipelines;


#[allow(dead_code)]
#[repr(C)]
#[derive(Default, Clone, Copy, BufferContents)]
pub struct CommonsUBO {
	projection: [PMat4; 2],
	view: [AMat4; 2],
	light_direction: [Vec4; 2],
	ambient: f32,
}

pub struct Renderer {
	pub instance: Arc<Instance>,
	pub device: Arc<Device>,
	pub load_queue: Arc<Queue>,
	pub render_pass: Arc<RenderPass>,
	pub queue: Arc<Queue>,
	pub pipelines: Pipelines,
	pub command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
	pub descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
	pub memory_allocator: Arc<StandardMemoryAllocator>,
	pub commons: Subbuffer<CommonsUBO>,
	
	future: Option<Box<dyn GpuFuture>>,
	debug_renderer: Option<DebugRenderer>,
	fps_counter: FpsCounter<20>,
	assets_manager: Option<AssetsManager>,
	transparent_registry: Option<Vec<(f32, u64)>>,
	#[allow(dead_code)] debug_utils_messenger: Option<DebugUtilsMessenger>,
}

pub const IMAGE_FORMAT: Format = Format::R8G8B8A8_SRGB;
pub const DEPTH_FORMAT: Format = Format::D24_UNORM_S8_UINT;
pub const LAYERS: u32 = 2;

impl Renderer {
	pub fn new(vr: Option<Arc<VR>>) -> Result<Renderer, RendererError> {
		let instance = Renderer::create_vulkan_instance(&vr)?;
		let debug_utils_messenger = config::get()
		                                   .validation
		                                   .then(|| Renderer::create_debug_callbacks(&instance))
		                                   .transpose()?;
		
		let physical = Renderer::create_physical_device(&instance, &vr)?;
		let (device, queue, load_queue) = Renderer::create_device(physical, &vr)?;
		
		let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(device.clone(), StandardCommandBufferAllocatorCreateInfo::default()));
		let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(device.clone()));
		let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
		let render_pass = Renderer::create_render_pass(&device)?;
		let mut pipelines = Pipelines::new(render_pass.clone());
		
		let commons = Buffer::new_sized(&memory_allocator,
		                                (BufferUsage::TRANSFER_DST | BufferUsage::UNIFORM_BUFFER).into_info(),
		                                MemoryUsage::DeviceOnly.into_info())?;
		
		let debug_renderer = Some(DebugRenderer::new(&load_queue, &memory_allocator, &command_buffer_allocator, &descriptor_set_allocator, &mut pipelines)?);
		let assets_manager = Some(AssetsManager::new());
		let fps_counter = FpsCounter::new();
		
		Ok(Renderer {
			instance,
			device,
			load_queue,
			render_pass,
			queue,
			pipelines,
			command_buffer_allocator,
			descriptor_set_allocator,
			memory_allocator,
			commons,
			future: None,
			debug_renderer,
			fps_counter,
			assets_manager,
			transparent_registry: None,
			debug_utils_messenger,
		})
	}
	
	fn create_vulkan_instance(vr: &Option<Arc<VR>>) -> Result<Arc<Instance>, RendererError> {
		let library = VulkanLibrary::new().unwrap();
		
		dprintln!("List of Vulkan layers available to use:");
		let layer_properties: Vec<_> = library.layer_properties()?.collect();
		for layer in &layer_properties {
			dprintln!("\t{}", layer.name());
		}
		
		let vr_extensions = vr.as_ref().map(|vr| vr.lock().unwrap().compositor.vulkan_instance_extensions_required()).unwrap_or_default();
		
		let extensions = InstanceExtensions::from_iter(vr_extensions.iter().map(|ex| ex.to_str().expect("Failed to parse OpenVR required instance extensions!")))
		                                    .union(&vulkano_win::required_extensions(&library))
		                                    .union(&InstanceExtensions {
			                                    ext_debug_utils: debug::debug(),
			                                    ext_debug_report: debug::debug(), // required by RenderDoc
			                                    khr_get_physical_device_properties2: true, // required by multiview
			                                    khr_external_semaphore_capabilities: true, // required by khr_external_semaphore from vr_extensions
			                                    ..InstanceExtensions::empty()
		                                    });
		
		let mut layers = vec![];
		
		if config::get().validation {
			layers.push("VK_LAYER_KHRONOS_validation".to_string());
		}
		
		let removed = layers.extract_if(|layer| layer_properties.iter().all(|al| al.name() != layer));
		
		for layer in removed {
			eprintln!("MISSING LAYER: {}", layer);
		}
		
		Ok(Instance::new(library,
		                 InstanceCreateInfo {
			                 engine_version: Version::V1_2,
			                 enabled_extensions: extensions,
			                 enabled_layers: layers,
			                 ..InstanceCreateInfo::application_from_cargo_toml()
		                 })?)
	}
	
	fn create_debug_callbacks(instance: &Arc<Instance>) -> Result<DebugUtilsMessenger, RendererError> {
		// SAFETY: user callback must not make any calls to the Vulkan API.
		unsafe {
			Ok(DebugUtilsMessenger::new(instance.clone(), DebugUtilsMessengerCreateInfo {
				message_severity: DebugUtilsMessageSeverity::ERROR
					| DebugUtilsMessageSeverity::WARNING
					| DebugUtilsMessageSeverity::INFO
					| DebugUtilsMessageSeverity::VERBOSE,
				message_type: DebugUtilsMessageType::GENERAL
					| DebugUtilsMessageType::VALIDATION
					| DebugUtilsMessageType::PERFORMANCE,
				..DebugUtilsMessengerCreateInfo::user_callback(Arc::new(|msg| {
					if !debug::debug() { return }
					
					let color = if msg.severity.contains(DebugUtilsMessageSeverity::ERROR) { colored::Color::BrightRed }
					else if msg.severity.contains(DebugUtilsMessageSeverity::WARNING) { colored::Color::Yellow }
					else if msg.severity.contains(DebugUtilsMessageSeverity::INFO) { colored::Color::Cyan }
					else { colored::Color::White };
					
					println!("[{} {}] {}: {}",
					         format!("{:?}", msg.ty).color(color).bold(),
					         format!("{:?}", msg.severity).color(color).bold(),
					         msg.layer_prefix.unwrap_or("UNKNOWN").bright_white(),
					         msg.description.white());
				}))
			})?)
		}
	}
	
	fn create_physical_device(instance: &Arc<Instance>, vr: &Option<Arc<VR>>) -> Result<Arc<PhysicalDevice>, RendererError> {
		dprintln!("Devices:");
		let physical_devices = instance.enumerate_physical_devices()?
		                               .collect::<Vec<_>>();
		
		for (index, physical_device) in physical_devices.iter().enumerate() {
			dprintln!("\t{}: {} api: {} driver: {}",
			          index,
			          physical_device.properties().device_name,
			          physical_device.properties().api_version,
			          physical_device.properties().driver_version);
		}
		
		let index = vr.as_ref()
		                 .and_then(|vr| vr.lock().unwrap().system.vulkan_output_device(instance.as_ptr()))
		                 .and_then(|ptr| physical_devices.iter().position(|physical| physical.as_ptr() == ptr))
		                 .unwrap_or_else(|| {
			                 if vr.is_some() { println!("Failed to fetch device from openvr, using fallback"); }
			                 config::get().gpu_id
		                 });
		
		let physical_device = physical_devices.into_iter()
		                                      .skip(index)
		                                      .next()
		                                      .ok_or(RendererError::NoDevices)?;
		
		if physical_device.properties().max_multiview_view_count.unwrap_or(0) < 2 {
			return Err(RendererError::MultiviewNotSupported);
		}
		
		dprintln!("\nUsing {}: {} api: {} driver: {}",
		          index,
		          physical_device.properties().device_name,
		          physical_device.properties().api_version,
		          physical_device.properties().driver_version);
		
		Ok(physical_device)
	}
	
	fn create_device(physical: Arc<PhysicalDevice>, vr: &Option<Arc<VR>>) -> Result<(Arc<Device>, Arc<Queue>, Arc<Queue>), RendererError> {
		for family in physical.queue_family_properties() {
			dprintln!("Found a queue family with {} queue(s) {:?}",
			          family.queue_count,
			          family.queue_flags);
		}
		
		let queue_family = physical.queue_family_properties()
		                           .iter()
		                           .position(|q| q.queue_flags.contains(QueueFlags::GRAPHICS))
		                           .ok_or(RendererError::NoQueue)?;
		
		// TODO: q.supports_graphics() prevents you from using pure transfer-oriented families, but it's required for mipmaps. Something has to be done about it.
		let load_queue_family = physical.queue_family_properties()
		                                .iter()
		                                .enumerate()
		                                .position(|(id, q)| q.queue_flags.contains(QueueFlags::TRANSFER) && q.queue_flags.contains(QueueFlags::GRAPHICS) && id != queue_family);
		
		let mut queue_create_infos = vec![QueueCreateInfo {
			queue_family_index: queue_family as u32,
			..QueueCreateInfo::default()
		}];
		
		if let Some(load_queue_family) = load_queue_family {
			queue_create_infos.push(QueueCreateInfo {
				queue_family_index: load_queue_family as u32,
				..QueueCreateInfo::default()
			});
		}
		
		let vr_extensions = vr.as_ref().map(|vr| vulkan_device_extensions_required(&vr.lock().unwrap().compositor, &physical)).unwrap_or_default();
		
		let (device, mut queues) = Device::new(physical, DeviceCreateInfo {
			enabled_extensions: DeviceExtensions::from_iter(vr_extensions.iter().map(|c| c.to_str().expect("Failed to parse OpenVR required device extensions!"))).union(&DeviceExtensions {
				khr_swapchain: true,
				khr_storage_buffer_storage_class: true,
				..DeviceExtensions::empty()
			}),
			enabled_features: Features {
				multiview: true,
				..Features::empty()
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
		let view_mask = (1 << LAYERS) - 1;
		
		let mut attachments = vec![
			AttachmentDescription {
				format: Some(IMAGE_FORMAT),
				samples,
				load_op: LoadOp::Clear,
				store_op: StoreOp::Store,
				initial_layout: ImageLayout::ColorAttachmentOptimal,
				final_layout: ImageLayout::ColorAttachmentOptimal,
				..AttachmentDescription::default()
			},
			AttachmentDescription {
				format: Some(DEPTH_FORMAT),
				samples,
				load_op: LoadOp::Clear,
				store_op: StoreOp::DontCare,
				stencil_load_op: LoadOp::Clear,
				stencil_store_op: StoreOp::DontCare,
				initial_layout: ImageLayout::DepthStencilAttachmentOptimal,
				final_layout: ImageLayout::DepthStencilAttachmentOptimal,
				..AttachmentDescription::default()
			},
		];
		
		let mut subpasses = vec![
			SubpassDescription {
				view_mask,
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
				format: Some(IMAGE_FORMAT),
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
			correlated_view_masks: vec![view_mask],
			..RenderPassCreateInfo::default()
		})?;
		
		Ok(render_pass)
	}
	
	pub fn create_swapchain(&self, surface: Arc<Surface>) -> Result<(Arc<Swapchain>, Vec<Arc<SwapchainImage>>), RendererSwapchainError> {
		if !self.device.physical_device().surface_support(self.queue.queue_family_index(), &surface)? {
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
		                 .find(|format| format.0 == Format::B8G8R8A8_SRGB || format.0 == Format::R8G8B8A8_SRGB)
		                 .expect("sRGB format not supported on the surface")
		                 .clone();
		
		let alpha_preference = [CompositeAlpha::PreMultiplied, CompositeAlpha::Opaque, CompositeAlpha::Inherit];
		let alpha = alpha_preference.iter()
		                            .copied()
		                            .find(|&composite| caps.supported_composite_alpha.contains_enum(composite))
		                            .expect("PreMultiplied and Opaque alpha composites not supported on the surface");
		
		Ok(Swapchain::new(self.device.clone(), surface, SwapchainCreateInfo {
			min_image_count: 2.max(caps.min_image_count).min(caps.max_image_count.unwrap_or(caps.min_image_count)),
			image_format: Some(format.0),
			image_color_space: format.1,
			image_extent: dimensions,
			image_usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
			composite_alpha: alpha,
			present_mode: PresentMode::Fifo,
			clipped: false,
			..SwapchainCreateInfo::default()
		})?)
	}
	
	pub fn create_framebuffer(&self, min_framebuffer_size: (u32, u32)) -> Result<FramebufferBundle, RendererCreateFramebufferError> {
		let config = config::get();
		let ssaa = config.ssaa;
		let samples = config.msaa.try_into().map_err(|_| RendererCreateFramebufferError::InvalidMultiSamplingCount(config.msaa))?;
		
		let dimensions = [
			(min_framebuffer_size.0 as f32 * ssaa) as u32,
			(min_framebuffer_size.1 as f32 * ssaa) as u32,
		];
		
		let main_image = AttachmentImage::multisampled_with_usage_with_layers(&self.memory_allocator,
		                                                                      dimensions,
		                                                                      LAYERS,
		                                                                      SampleCount::Sample1,
		                                                                      IMAGE_FORMAT,
		                                                                      ImageUsage::COLOR_ATTACHMENT
			                                                                      | ImageUsage::TRANSFER_SRC
			                                                                      | ImageUsage::TRANSFER_DST
			                                                                      | ImageUsage::SAMPLED)?;
		
		let depth_image = AttachmentImage::multisampled_with_usage_with_layers(&self.memory_allocator,
		                                                                       dimensions,
		                                                                       LAYERS,
		                                                                       samples,
		                                                                       DEPTH_FORMAT,
		                                                                       ImageUsage::DEPTH_STENCIL_ATTACHMENT
			                                                                       | ImageUsage::TRANSIENT_ATTACHMENT)?;
		
		
		
		let attachments: Vec<Arc<dyn ImageViewAbstract>> = if samples == SampleCount::Sample1 {
			vec![
				ImageView::new_default(main_image.clone())?,
				ImageView::new_default(depth_image)?,
			]
		} else {
			let msaa_image = AttachmentImage::multisampled_with_usage_with_layers(&self.memory_allocator,
			                                                                      dimensions,
			                                                                      LAYERS,
			                                                                      samples,
			                                                                      IMAGE_FORMAT,
			                                                                      ImageUsage::COLOR_ATTACHMENT)?;
			
			vec![
				ImageView::new_default(msaa_image)?,
				ImageView::new_default(depth_image)?,
				ImageView::new_default(main_image.clone())?,
			]
		};
		
		let framebuffer = Framebuffer::new(self.render_pass.clone(), FramebufferCreateInfo {
			attachments,
			extent: dimensions,
			..FramebufferCreateInfo::default()
		})?;
		
		let mut clear_values = vec![ Some(ClearValue::Float([0.0, 0.0, 0.0, 0.0])) ];
		
		if DEPTH_FORMAT.type_stencil().is_some() {
			clear_values.push(Some(ClearValue::DepthStencil((1.0, 0))))
		} else {
			clear_values.push(Some(ClearValue::Depth(1.0)))
		}
		
		if samples != SampleCount::Sample1 {
			clear_values.push(None)
		}
		
		Ok(FramebufferBundle {
			framebuffer,
			main_image,
			ssaa,
			clear_values
		})
	}
	
	pub fn begin_frame(&mut self) -> Result<(), RendererBeginFrameError> {
		if let Some(previous_frame) = self.future.take() {
			// TODO: double fence?
			previous_frame.then_signal_fence_and_flush()?
			              .wait(None)?;
		}
		
		self.fps_counter.tick();
		
		debug::draw_text(format!("FPS: {}", self.fps_counter.fps().floor()), point!(-1.0, -1.0), debug::DebugOffset::bottom_right(16.0, 16.0), 64.0, Color::GREEN);
		debug::draw_text(format!("CAM FPS: {}", debug::get_flag_or_default::<f32>("CAMERA_FPS").floor()), point!(-1.0, -1.0), debug::DebugOffset::bottom_right(16.0, 96.0), 64.0, Color::GREEN);
		
		self.future = Some(sync::now(self.device.clone()).boxed());
		
		Ok(())
	}
	
	pub fn enqueue<F>(&mut self, queue: Arc<Queue>, callback: F)
	                  where F: FnOnce(Box<dyn GpuFuture>) -> Box<dyn GpuFuture> {
		self.try_enqueue(queue, |future| Ok::<_, !>(callback(future))).unwrap()
	}
	
	pub fn try_enqueue<Err, F>(&mut self, queue: Arc<Queue>, callback: F)
	                           -> Result<(), Err>
	                           where F: FnOnce(Box<dyn GpuFuture>) -> Result<Box<dyn GpuFuture>, Err> {
		let mut future = self.future.take().unwrap_or_else(|| sync::now(self.device.clone()).boxed());
		if !future.queue_change_allowed() && future.queue().unwrap() != queue {
			future = future.then_signal_semaphore().boxed();
		}
		
		future = callback(future)?;
		
		self.future = Some(future);
		
		Ok(())
	}
	
	pub fn render<RT>(&mut self, camera_pos: Isometry3, scene: &mut HashMap<u64, Entity>, render_target: &mut RT)
	                  -> Result<(), RendererRenderError<RT::RenderError>>
	                  where RT: RenderTarget {
		let rt_context = match render_target.create_context(camera_pos) {
			Ok(Some(context)) => context,
			Ok(None) => return Ok(()),
			Err(err) => return Err(RendererRenderError::RenderTargetError(err)),
		};
		
		let mut builder = AutoCommandBufferBuilder::primary(&*self.command_buffer_allocator, self.queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit)?;
		
		let light_source = vector!(0.5, -0.5, -1.5).normalize();
		builder.update_buffer(self.commons.clone(), Arc::new(CommonsUBO {
			projection: [rt_context.projection.0.into(), rt_context.projection.1.into()],
			view: [rt_context.view.0.into(), rt_context.view.1.into()],
			light_direction: [
				(rt_context.view.0 * light_source).to_homogeneous().into(),
				(rt_context.view.1 * light_source).to_homogeneous().into(),
			],
			ambient: 0.25,
		}))?;
		
		let mut context = RenderContext::new(&rt_context, &mut builder, camera_pos);
		
		let mut transparent_registry = self.transparent_registry.take().unwrap_or_default();
		
		for entity in scene.values_mut() {
			let transparent = entity.before_render(&mut context, self)?;
			
			if transparent {
				transparent_registry.push(((entity.state().position.translation.vector - camera_pos.translation.vector).magnitude(), entity.id));
			}
		}
		
		{
			let mut debug_renderer = self.debug_renderer.take().unwrap();
			debug_renderer.before_render(self)?;
			self.debug_renderer = Some(debug_renderer);
		}
		
		render_target.before_render(&mut context, self).map_err(RendererRenderError::RenderTargetError)?;
		
		let viewport = Viewport {
			origin: [0.0, 0.0],
			dimensions: [context.framebuffer_size.0 as f32, context.framebuffer_size.1 as f32],
			depth_range: 0.0..1.0,
		};
		context.builder.begin_render_pass(RenderPassBeginInfo {
			                                  clear_values: render_target.clear_values().iter().copied().collect(),
			                                  ..RenderPassBeginInfo::framebuffer(rt_context.framebuffer.clone())
		                                  }, SubpassContents::Inline)?
		               .set_viewport(0, Some(viewport));
		
		render_target.early_render(&mut context, self).map_err(RendererRenderError::RenderTargetError)?;
		
		context.render_type = RenderType::Opaque;
		
		for entity in scene.values_mut() {
			entity.render(&mut context, self)?;
		}
		
		transparent_registry.sort_unstable_by(|(a, _), (b, _)| b.partial_cmp(a).unwrap());
		
		context.render_type = RenderType::Transparent;
		
		for (_, entity) in transparent_registry.iter() {
			scene.get_mut(&entity).unwrap().render(&mut context, self)?;
		}
		
		transparent_registry.clear();
		self.transparent_registry = Some(transparent_registry);
		
		self.debug_renderer.as_mut().unwrap().render(&mut context)?;
		
		render_target.late_render(&mut context, self).map_err(RendererRenderError::RenderTargetError)?;
		
		context.builder.end_render_pass()?;
		
		render_target.after_render(&mut context, self).map_err(RendererRenderError::RenderTargetError)?;
		
		drop(context);
		
		let command_buffer = builder.build()?;
		
		let queue = self.queue.clone();
		self.try_enqueue(queue.clone(), |future| future.then_execute(queue.clone(), command_buffer).map(GpuFuture::boxed))?;
		
		render_target.after_execute(self).map_err(RendererRenderError::RenderTargetError)?;
		
		Ok(())
	}
	
	pub fn end_frame(&mut self) -> Result<(), RendererEndFrameError> {
		self.debug_renderer.as_mut().unwrap().reset();
		
		if let Some(future) = self.future.take() {
			if future.queue().is_none() {
				return Ok(())
			}
			
			match future.then_signal_fence_and_flush() {
				Ok(future) => {
					self.future = Some(future.boxed());
				},
				Err(sync::FlushError::OutOfDate) => {
					// ignore
				},
				Err(err) => return Err(err.into()),
			}
		}
		
		Ok(())
	}
	
	pub fn debug_text_cache(&self) -> RefMut<TextCache> {
		self.debug_renderer.as_ref().unwrap().text_cache()
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
	#[error(display = "{}", _0)] DebugRendererError(#[error(source)] DebugRendererError),
	#[error(display = "{}", _0)] BufferError(#[error(source)] BufferError),
	#[error(display = "{}", _0)] OomError(#[error(source)] vulkano::OomError),
	#[error(display = "{}", _0)] VulkanError(#[error(source)] vulkano::VulkanError),
	#[error(display = "{}", _0)] InstanceCreationError(#[error(source)] instance::InstanceCreationError),
	#[error(display = "{}", _0)] DebugUtilsMessengerCreationError(#[error(source)] instance::debug::DebugUtilsMessengerCreationError),
	#[error(display = "{}", _0)] DeviceCreationError(#[error(source)] device::DeviceCreationError),
	#[error(display = "{}", _0)] RenderPassCreationError(#[error(source)] render_pass::RenderPassCreationError),
}

#[derive(Debug, Error)]
pub enum RendererSwapchainError {
	#[error(display = "Surface presentation is not supported.")] SurfaceNotSupported,
	#[error(display = "{}", _0)] PhysicalDeviceError(#[error(source)] physical::PhysicalDeviceError),
	#[error(display = "{}", _0)] SwapchainCreationError(#[error(source)] swapchain::SwapchainCreationError),
}

#[derive(Debug, Error)]
pub enum RendererCreateFramebufferError {
	#[error(display = "Invalid Multi-Sampling count: {}", _0)] InvalidMultiSamplingCount(u32),
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] vulkano::image::ImageError),
	#[error(display = "{}", _0)] FramebufferCreationError(#[error(source)] render_pass::FramebufferCreationError),
}

#[derive(Debug, Error)]
pub enum RendererBeginFrameError {
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
}

#[derive(Debug, Error)]
pub enum RendererEndFrameError {
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
}

#[derive(Debug, Error)]
pub enum RendererRenderError<RTE: Error> {
	#[error(display = "{}", _0)] RenderTargetError(RTE),
	#[error(display = "{}", _0)] ComponentError(ComponentError),
	#[error(display = "{}", _0)] DebugRendererPreRenderError(#[error(source)] DebugRendererPreRenderError),
	#[error(display = "{}", _0)] DebugRendererRenderError(#[error(source)] DebugRendererRenderError),
	#[error(display = "{}", _0)] OomError(#[error(source)] vulkano::OomError),
	#[error(display = "{}", _0)] RenderPassError(#[error(source)] command_buffer::RenderPassError),
	#[error(display = "{}", _0)] CommandBufferBeginError(#[error(source)] command_buffer::CommandBufferBeginError),
	#[error(display = "{}", _0)] PipelineExecutionError(#[error(source)] command_buffer::PipelineExecutionError),
	#[error(display = "{}", _0)] CommandBufferBuildError(#[error(source)] command_buffer::BuildError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] command_buffer::CommandBufferExecError),
	#[error(display = "{}", _0)] CopyError(#[error(source)] command_buffer::CopyError),
	#[error(display = "{}", _0)] ClearError(#[error(source)] command_buffer::ClearError),
}

impl<RTE: Error> From<ComponentError> for RendererRenderError<RTE> {
	fn from(err: ComponentError) -> Self {
		RendererRenderError::ComponentError(err)
	}
}
