use std::sync::Arc;
use err_derive::Error;
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract};
use vulkano::image::{AttachmentImage, ImageUsage, ImageAccess};
use vulkano::format::Format;
use vulkano::format;
use vulkano::device::Queue;
use openvr::compositor::texture::{vulkan, Handle, ColorSpace};
use openvr::compositor::Texture;

use crate::utils::{OpenVRPtr};
use crate::application::VR;
use crate::math::{Mat4, Perspective3, ToTransform, AMat4, VRSlice, PMat4, SubsetOfLossy};
use super::RenderPass;

// Translates OpenGL projection matrix to Vulkan
// Can't be const because Mat4::new is not const fn or something
fn clip() -> AMat4 {
	AMat4::from_matrix_unchecked(Mat4::new(
		1.0, 0.0, 0.0, 0.0,
		0.0,-1.0, 0.0, 0.0,
		0.0, 0.0, 0.5, 0.5,
		0.0, 0.0, 0.0, 1.0,
	))
}

pub struct Eyes {
	pub left: Eye,
	pub right: Eye,
	pub frame_buffer_size: (u32, u32)
}

impl Eyes {
	pub fn new(queue: &Arc<Queue>, render_pass: &Arc<RenderPass>) -> Result<Eyes, EyeCreationError> {
		let frame_buffer_size = (960, 1080);
		let view = AMat4::identity();
		let projection = clip() * Perspective3::new(frame_buffer_size.0 as f32 / frame_buffer_size.1 as f32, 90.0 / 360.0 * std::f32::consts::TAU, 0.01, 1000.0).as_projective();
		
		Ok(Eyes {
			left: Eye::new(frame_buffer_size, view.clone(), projection, queue, render_pass)?,
			right: Eye::new(frame_buffer_size, view.clone(), projection, queue, render_pass)?,
			frame_buffer_size,
		})
	}
	
	pub fn new_vr(vr: &VR, queue: &Arc<Queue>, render_pass: &Arc<RenderPass>) -> Result<Eyes, EyeCreationError> {
		let frame_buffer_size = vr.system.recommended_render_target_size();
		
		let view_left  = vr.system.eye_to_head_transform(openvr::Eye::Left ).to_transform().inverse();
		let view_right = vr.system.eye_to_head_transform(openvr::Eye::Right).to_transform().inverse();
		
		let proj_left  = clip() * PMat4::from_superset_lossy(&Mat4::from_slice44(&vr.system.projection_matrix(openvr::Eye::Left,  0.1, 1000.1)));
		let proj_right = clip() * PMat4::from_superset_lossy(&Mat4::from_slice44(&vr.system.projection_matrix(openvr::Eye::Right, 0.1, 1000.1)));
		
		Ok(Eyes {
			left: Eye::new(frame_buffer_size, view_left, proj_left, queue, render_pass)?,
			right: Eye::new(frame_buffer_size, view_right, proj_right, queue, render_pass)?,
			frame_buffer_size,
		})
	}
}


pub struct Eye {
	pub image: Arc<AttachmentImage<format::R8G8B8A8Srgb>>,
	pub depth_image: Arc<AttachmentImage<format::D16Unorm>>,
	pub texture: Texture,
	pub view: AMat4,
	pub projection: PMat4,
	pub frame_buffer: Arc<dyn FramebufferAbstract + Send + Sync>,
}

pub const IMAGE_FORMAT: Format = Format::R8G8B8A8Srgb;
pub const DEPTH_FORMAT: Format = Format::D16Unorm;

impl Eye {
	pub fn new<RPD>(frame_buffer_size:(u32, u32), view: AMat4, projection: PMat4, queue: &Queue, render_pass: &Arc<RPD>)
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
			view,
			projection,
			frame_buffer,
		})
	}
}

#[derive(Debug, Error)]
pub enum EyeCreationError {
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] vulkano::image::ImageCreationError),
	#[error(display = "{}", _0)] FramebufferCreationError(#[error(source)] vulkano::framebuffer::FramebufferCreationError),
}
