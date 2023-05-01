use std::sync::Arc;
use err_derive::Error;
use openvr::compositor::texture::{ColorSpace, Handle, vulkan};
use openvr::compositor::Texture;
use vulkano::{command_buffer, sync};
use vulkano::command_buffer::{CopyImageInfo, ImageCopy};
use vulkano::image::{AttachmentImage, ImageAccess, ImageDimensions, ImageSubresourceLayers, ImageUsage, SampleCount, StorageImage, ImageCreateFlags};
use vulkano::format::ClearValue;
use vulkano::sync::GpuFuture;

pub mod camera;
// mod openvr_cb;
mod background;
mod pipeline;

use crate::debug;
use crate::utils::{default_tracked_pose, FramebufferBundle, OpenVRPtr};
use crate::math::{AMat4, Isometry3, Mat4, Perspective3, PI, PMat4, projective_clip, SubsetOfLossy, Vec4, VRSlice};
use crate::config::NovrConfig;
use crate::renderer::{IMAGE_FORMAT, RenderContext, Renderer, RendererCreateFramebufferError, RenderTarget, RenderTargetContext};
use super::VR;
use camera::Camera;
// use openvr_cb::OpenVRCommandBuffer;
use background::{Background, BackgroundError, BackgroundRenderError, BackgroundLoadError};


pub struct Eyes {
	fb: FramebufferBundle,
	side_image: Arc<StorageImage>, // TODO: https://github.com/ValveSoftware/openvr/issues/663
	textures: (Texture, Texture),
	view: (AMat4, AMat4),
	projection: (PMat4, PMat4),
	raw_projection: (Vec4, Vec4),
	vr: Option<Arc<VR>>,
	background: Option<Background>,
	hmd_pose: [[f32; 4]; 3],
}


impl Eyes {
	pub fn new_novr(config: &NovrConfig, camera: Option<Box<dyn Camera>>, renderer: &mut Renderer) -> Result<Eyes, EyesCreationError> {
		let min_framebuffer_size = (config.frame_buffer_width, config.frame_buffer_height);
		let aspect = config.frame_buffer_width as f32 / config.frame_buffer_height as f32;
		let fovx = config.fov / 180.0 * PI;
		let fovy = ((fovx / 2.0).tan() / aspect).atan() * 2.0;
		
		let view = AMat4::identity();
		let projection = projective_clip() * Perspective3::new(aspect, fovy, 0.1, 100.0).as_projective();
		let raw = vector!((fovx / 2.0).tan(), (fovx / 2.0).tan(), (fovy / 2.0).tan(), (fovy / 2.0).tan());
		
		Self::new(min_framebuffer_size, (view, view), (projection, projection), (raw, raw), None, camera, renderer)
	}
	
	pub fn new_vr(vr: Arc<VR>, camera: Option<Box<dyn Camera>>, renderer: &mut Renderer) -> Result<Eyes, EyesCreationError> {
		let vr_lock = vr.lock().unwrap();
		let min_framebuffer_size = vr_lock.system.recommended_render_target_size();
		
		let view_left  = AMat4::from_superset_lossy(&Mat4::from_slice34(&vr_lock.system.eye_to_head_transform(openvr::Eye::Left ))).inverse();
		let view_right = AMat4::from_superset_lossy(&Mat4::from_slice34(&vr_lock.system.eye_to_head_transform(openvr::Eye::Right))).inverse();
		
		let proj_left  = projective_clip() * PMat4::from_superset_lossy(&Mat4::from_slice44(&vr_lock.system.projection_matrix(openvr::Eye::Left, 0.1, 100.0)));
		let proj_right = projective_clip() * PMat4::from_superset_lossy(&Mat4::from_slice44(&vr_lock.system.projection_matrix(openvr::Eye::Right, 0.1, 100.0)));
		
		let raw_left  = vr_lock.system.projection_raw(openvr::Eye::Left);
		let raw_left = vector!(-raw_left.left, raw_left.right, -raw_left.top, raw_left.bottom);
		
		let raw_right = vr_lock.system.projection_raw(openvr::Eye::Right);
		let raw_right = vector!(-raw_right.left, raw_right.right, -raw_right.top, raw_right.bottom);
		
		drop(vr_lock);
		
		Self::new(min_framebuffer_size, (view_left, view_right), (proj_left, proj_right), (raw_left, raw_right), Some(vr), camera, renderer)
	}
	
	pub fn new(min_framebuffer_size: (u32, u32), view: (AMat4, AMat4), projection: (PMat4, PMat4), raw_projection: (Vec4, Vec4), vr: Option<Arc<VR>>, camera: Option<Box<dyn Camera>>, renderer: &mut Renderer) -> Result<Eyes, EyesCreationError> {
		let background = camera.map(|camera| Background::new(camera, raw_projection, renderer))
		                       .transpose()?;
		
		let fb = renderer.create_framebuffer(min_framebuffer_size)?;
		
		let dimensions = fb.framebuffer.extent();
		
		let side_image = StorageImage::with_usage(&renderer.memory_allocator,
		                                          ImageDimensions::Dim2d {
			                                          width: dimensions[0],
			                                          height: dimensions[1],
			                                          array_layers: 1
		                                          },
		                                          IMAGE_FORMAT,
		                                          ImageUsage::TRANSFER_SRC
			                                          | ImageUsage::TRANSFER_DST
			                                          | ImageUsage::SAMPLED,
		                                          ImageCreateFlags::empty(),
		                                          Some(renderer.queue.queue_family_index()))?;
		
		let handle_defs = vulkan::Texture {
			image: 0,
			device: renderer.device.as_ptr(),
			physical_device: renderer.device.physical_device().as_ptr(),
			instance: renderer.instance.as_ptr(),
			queue: renderer.queue.as_ptr(),
			queue_family_index: renderer.queue.queue_family_index(),
			width: fb.main_image.dimensions().width(),
			height: fb.main_image.dimensions().height(),
			format: fb.main_image.format() as u32,
			sample_count: 1,
		};
		
		let left_texture = Texture {
			handle: Handle::Vulkan(vulkan::Texture {
				image: (*fb.main_image).as_ptr(),
				..handle_defs
			}),
			color_space: ColorSpace::Gamma,
		};
		
		let right_texture = Texture {
			handle: Handle::Vulkan(vulkan::Texture {
				image: (*side_image).as_ptr(),
				..handle_defs
			}),
			color_space: ColorSpace::Gamma,
		};
		
		Ok(Eyes {
			fb,
			side_image,
			textures: (left_texture, right_texture),
			view,
			projection,
			raw_projection,
			vr,
			background,
			hmd_pose: default_tracked_pose().device_to_absolute_tracking().clone(),
		})
	}
	
	pub fn load_background(&mut self, camera_pos: Isometry3, renderer: &mut Renderer) -> Result<(), EyesLoadBackgroundError> {
		if let Some(background) = &mut self.background {
			renderer.try_enqueue(renderer.load_queue.clone(), |future| background.load_camera(camera_pos, future))?;
		}
		
		Ok(())
	}
	
	pub fn set_hmd_pose(&mut self, hmd_pose: [[f32; 4]; 3]) {
		self.hmd_pose = hmd_pose;
	}
	
	pub fn framebuffer_size(&self) -> (u32, u32) {
		self.fb.size()
	}
}

impl RenderTarget for Eyes {
	type RenderError = EyesRenderTargetError;
	
	fn create_context(&mut self, camera_pos: Isometry3) -> Result<Option<RenderTargetContext>, Self::RenderError> {
		let center_pos = camera_pos.inverse();
		
		Ok(Some(RenderTargetContext::new(self.fb.clone(),
		                                 (self.view.0 * center_pos, self.view.1 * center_pos),
		                                 self.projection,
		                                 (
			                                 vector!(self.raw_projection.0[0].atan() + self.raw_projection.0[1].atan(), self.raw_projection.0[2].atan() + self.raw_projection.0[3].atan()),
			                                 vector!(self.raw_projection.1[0].atan() + self.raw_projection.1[1].atan(), self.raw_projection.1[2].atan() + self.raw_projection.1[3].atan())
		                                 ))))
	}
	
	fn clear_values(&self) -> &[Option<ClearValue>] {
		&self.fb.clear_values
	}
	
	fn last_frame(&self) -> &Arc<AttachmentImage> {
		&self.fb.main_image
	}
	
	fn early_render(&mut self, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<(), EyesRenderTargetError> {
		if let Some(background) = &mut self.background {
			background.render(context.camera_pos, context.builder)?;
		}
		
		Ok(())
	}
	
	fn after_render(&mut self, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<(), EyesRenderTargetError> {
		let framebuffer_size = self.framebuffer_size();
		
		let mut copy_info = CopyImageInfo::images(self.fb.main_image.clone(), self.side_image.clone());
		
		copy_info.regions.clear();
		copy_info.regions.push(ImageCopy {
			src_subresource: ImageSubresourceLayers {
				array_layers: 1..2,
				..self.fb.main_image.subresource_layers()
			},
			dst_subresource: ImageSubresourceLayers {
				array_layers: 0..1,
				..self.fb.main_image.subresource_layers()
			},
			extent: [framebuffer_size.0, framebuffer_size.1, 1],
			..ImageCopy::default()
		});
		
		context.builder.copy_image(copy_info)?;
		
		Ok(())
	}
	
	fn after_execute(&mut self, renderer: &mut Renderer) -> Result<(), EyesRenderTargetError> {
		// TODO: Explicit timing mode
		if let Some(ref vr) = self.vr {
			let vr = vr.lock().unwrap();
			
			// TODO: find way to change image layouts
			unsafe {
				let debug = debug::debug();
				if debug { debug::set_debug(false); } // Hide internal OpenVR warnings (https://github.com/ValveSoftware/openvr/issues/818)
				vr.compositor.submit(openvr::Eye::Left,  &self.textures.0, None, Some(self.hmd_pose))?;
				vr.compositor.submit(openvr::Eye::Right, &self.textures.1, None, Some(self.hmd_pose))?;
				if debug { debug::set_debug(true); }
			}
			
			// let queue = renderer.queue.clone();
			// let command_buffer_allocator = renderer.command_buffer_allocator.clone();
			//
			// renderer.try_enqueue::<EyesRenderTargetError, _>(queue.clone(), |future| {
			// 	// Safety: OpenVRCommandBuffer::end must be executed(flused) after start to not leave eye textures in an unexpected layout
			// 	let result = try { unsafe {
			// 		let f = future.then_execute(queue.clone(), OpenVRCommandBuffer::start(&*command_buffer_allocator, self.fb.main_image.clone(), queue.queue_family_index())?)?
			// 		              .then_execute(queue.clone(), OpenVRCommandBuffer::start(&*command_buffer_allocator, self.side_image.clone(), queue.queue_family_index())?)?;
			// 		f.flush()?;
			//
			// 		// let debug = debug::debug();
			// 		// if debug { debug::set_debug(false); } // Hide internal OpenVR warnings (https://github.com/ValveSoftware/openvr/issues/818)
			// 		vr.compositor.submit(openvr::Eye::Left,  &self.textures.0, None, Some(self.hmd_pose))?;
			// 		vr.compositor.submit(openvr::Eye::Right, &self.textures.1, None, Some(self.hmd_pose))?;
			// 		// if debug { debug::set_debug(true); }
			//
			// 		println!("End of VR enqueue");
			//
			// 		f.then_execute(queue.clone(), OpenVRCommandBuffer::end(&*command_buffer_allocator, self.fb.main_image.clone(), queue.queue_family_index())?)?
			// 		 .then_execute(queue.clone(), OpenVRCommandBuffer::end(&*command_buffer_allocator, self.side_image.clone(), queue.queue_family_index())?)?
			// 		 .boxed()
			// 	}};
			//
			// 	if let Err(error) = result {
			// 		panic!("Error while submitting VR frame!\n{}", error);
			// 	}
			//
			// 	result
			// })?;
		}
		
		Ok(())
	}
}

#[derive(Debug, Error)]
pub enum EyesCreationError {
	#[error(display = "{}", _0)] BackgroundError(#[error(source)] BackgroundError),
	#[error(display = "{}", _0)] RendererCreateFramebufferError(#[error(source)] RendererCreateFramebufferError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] vulkano::image::ImageError),
	#[error(display = "{}", _0)] ImmutableImageCreationError(#[error(source)] vulkano::image::immutable::ImmutableImageCreationError),
}

pub type EyesLoadBackgroundError = BackgroundLoadError;

#[derive(Debug, Error)]
pub enum EyesRenderTargetError {
	#[error(display = "{}", _0)] BackgroundRenderError(#[error(source)] BackgroundRenderError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] CopyError(#[error(source)] command_buffer::CopyError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] command_buffer::CommandBufferExecError),
	#[error(display = "{}", _0)] OomError(#[error(source)] vulkano::OomError),
	#[error(display = "{}", _0)] CompositorError(#[error(source)] openvr::compositor::CompositorError),
}
