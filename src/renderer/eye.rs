use std::sync::Arc;
use err_derive::Error;
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, FramebufferCreationError, RenderPassAbstract};
use vulkano::image::{AttachmentImage, ImageUsage, ImageAccess, ImageCreationError};
use vulkano::format::Format;
use vulkano::format;
use vulkano::device::Queue;
use openvr::compositor::texture::{vulkan, Handle, ColorSpace};
use openvr::compositor::Texture;
use cgmath::{Matrix4, Matrix, Deg, Transform};

use crate::utils::{OpenVRPtr, mat4};
use crate::application::VR;
use super::RenderPass;

// Translates OpenGL projection matrix to Vulkan
const CLIP: Matrix4<f32> = Matrix4::new(
	1.0, 0.0, 0.0, 0.0,
	0.0,-1.0, 0.0, 0.0,
	0.0, 0.0, 0.5, 0.0,
	0.0, 0.0, 0.5, 1.0,
);

pub struct Eyes {
	pub left: Eye,
	pub right: Eye,
	pub frame_buffer_size: (u32, u32)
}

impl Eyes {
	pub fn new(queue: &Arc<Queue>, render_pass: &Arc<RenderPass>) -> Result<Eyes, EyeCreationError> {
		let frame_buffer_size = (1920, 1080);
		let proj_left = CLIP * cgmath::perspective(Deg(90.0), frame_buffer_size.0 as f32 / frame_buffer_size.1 as f32, 0.1, 1000.0);
		let proj_right = CLIP * cgmath::perspective(Deg(90.0), frame_buffer_size.0 as f32 / frame_buffer_size.1 as f32, 0.1, 1000.0);
		
		Ok(Eyes {
			left: Eye::new(frame_buffer_size, proj_left, queue, render_pass)?,
			right: Eye::new(frame_buffer_size, proj_right, queue, render_pass)?,
			frame_buffer_size,
		})
	}
	
	pub fn new_vr(vr: &VR, queue: &Arc<Queue>, render_pass: &Arc<RenderPass>) -> Result<Eyes, EyeCreationError> {
		let frame_buffer_size = vr.system.recommended_render_target_size();
		let proj_left:  Matrix4<f32> = CLIP
		                             * Matrix4::from(vr.system.projection_matrix(openvr::Eye::Left,  0.1, 1000.1)).transpose()
		                             * mat4(&vr.system.eye_to_head_transform(openvr::Eye::Left )).inverse_transform().unwrap();
		let proj_right: Matrix4<f32> = CLIP
		                             * Matrix4::from(vr.system.projection_matrix(openvr::Eye::Right, 0.1, 1000.1)).transpose()
		                             * mat4(&vr.system.eye_to_head_transform(openvr::Eye::Right)).inverse_transform().unwrap();
		
		Ok(Eyes {
			left: Eye::new(frame_buffer_size, proj_left, queue, render_pass)?,
			right: Eye::new(frame_buffer_size, proj_right, queue, render_pass)?,
			frame_buffer_size,
		})
	}
}


pub struct Eye {
	pub image: Arc<AttachmentImage<format::R8G8B8A8Srgb>>,
	pub depth_image: Arc<AttachmentImage<format::D16Unorm>>,
	pub texture: Texture,
	pub projection: Matrix4<f32>,
	pub frame_buffer: Arc<dyn FramebufferAbstract + Send + Sync>,
}

pub const IMAGE_FORMAT: Format = Format::R8G8B8A8Srgb;
pub const DEPTH_FORMAT: Format = Format::D16Unorm;

impl Eye {
	pub fn new<RPD>(frame_buffer_size:(u32, u32), projection: Matrix4<f32>, queue: &Queue, render_pass: &Arc<RPD>)
	                -> Result<Eye, EyeCreationError>
	               where RPD: RenderPassAbstract + Sync + Send + 'static + ?Sized {
		let dimensions = [frame_buffer_size.0, frame_buffer_size.1];
		
		let device = queue.device();
		
		let image = AttachmentImage::with_usage(device.clone(),
		                                        dimensions,
		                                        format::R8G8B8A8Srgb,
		                                        ImageUsage { transfer_source: true,
		                                                     transfer_destination: true,
		                                                     sampled: true,
		                                                     ..ImageUsage::none() })?;
		
		let depth_image = AttachmentImage::transient(device.clone(), dimensions, format::D16Unorm)?;
		
		let texture = Texture {
			handle: Handle::Vulkan(vulkan::Texture {
				        image: (*image).as_ptr(),
				        device: device.as_ptr(),
				        physical_device: device.physical_device().as_ptr(),
				        instance: device.instance().as_ptr(),
				        queue: queue.as_ptr(),
				        queue_family_index: queue.family().id(),
				        width: image.dimensions().width(),
				        height: image.dimensions().height(),
				        format: image.format() as u32,
				        sample_count: image.samples(),
			        }),
			color_space: ColorSpace::Gamma,
		};
		
		
		let frame_buffer = Arc::new(Framebuffer::start(render_pass.clone())
		                       .add(image.clone())?
		                       .add(depth_image.clone())?
		                       .build()?);
		
		Ok(Eye {
			image,
			depth_image,
			texture,
			projection,
			frame_buffer,
		})
	}
}

#[derive(Debug, Error)]
pub enum EyeCreationError {
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] ImageCreationError),
	#[error(display = "{}", _0)] FramebufferCreationError(#[error(source)] FramebufferCreationError),
}
