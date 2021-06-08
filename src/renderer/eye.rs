use std::sync::Arc;
use err_derive::Error;
use vulkano::render_pass::{Framebuffer, FramebufferAbstract, RenderPass};
use vulkano::image::{AttachmentImage, ImageUsage, ImageAccess, view::ImageView, ImageDimensions};
use vulkano::format::{Format, ClearValue};
use vulkano::device::Queue;
use openvr::compositor::texture::{vulkan, Handle, ColorSpace};
use openvr::compositor::Texture;

use crate::config;
use crate::utils::OpenVRPtr;
use crate::application::VR;
use crate::math::{Mat4, Perspective3, ToTransform, AMat4, VRSlice, PMat4, SubsetOfLossy};
use crate::config::NovrConfig;

pub const IMAGE_FORMAT: Format = Format::R8G8B8A8Srgb;
pub const DEPTH_FORMAT: Format = Format::D16Unorm;

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
	pub main_image: Arc<AttachmentImage>,
	pub resolved_image: Arc<AttachmentImage>,
	pub side_image: Arc<AttachmentImage>, // TODO: https://github.com/ValveSoftware/openvr/issues/663
	pub depth_image: Arc<AttachmentImage>,
	pub frame_buffer: Arc<dyn FramebufferAbstract + Send + Sync>,
	pub frame_buffer_size: (u32, u32),
	pub textures: (Texture, Texture),
	pub view: (AMat4, AMat4),
	pub projection: (PMat4, PMat4),
	pub clear_values: Vec<ClearValue>,
}


impl Eyes {
	pub fn new_novr(config: &NovrConfig, queue: &Arc<Queue>, render_pass: &Arc<RenderPass>) -> Result<Eyes, EyeCreationError> {
		let min_frame_buffer_size = (config.frame_buffer_width, config.frame_buffer_height);
		let view = AMat4::identity();
		let projection = clip() * Perspective3::new(config.frame_buffer_width as f32 / config.frame_buffer_height as f32, config.fov / 360.0 * std::f32::consts::TAU, 0.01, 100.01).as_projective();
		
		Self::new(min_frame_buffer_size, (view.clone(), view), (projection, projection), queue, render_pass)
	}
	
	pub fn new_vr(vr: &VR, queue: &Arc<Queue>, render_pass: &Arc<RenderPass>) -> Result<Eyes, EyeCreationError> {
		let vr = vr.lock().unwrap();
		let min_frame_buffer_size = vr.system.recommended_render_target_size();
		
		let view_left  = vr.system.eye_to_head_transform(openvr::Eye::Left ).to_transform().inverse();
		let view_right = vr.system.eye_to_head_transform(openvr::Eye::Right).to_transform().inverse();
		
		let proj_left  = clip() * PMat4::from_superset_lossy(&Mat4::from_slice44(&vr.system.projection_matrix(openvr::Eye::Left,  0.01, 100.01)));
		let proj_right = clip() * PMat4::from_superset_lossy(&Mat4::from_slice44(&vr.system.projection_matrix(openvr::Eye::Right, 0.01, 100.01)));
		
		Self::new(min_frame_buffer_size, (view_left, view_right), (proj_left, proj_right), queue, render_pass)
	}
	
	pub fn new(min_frame_buffer_size: (u32, u32), view: (AMat4, AMat4), projection: (PMat4, PMat4), queue: &Queue, render_pass: &Arc<RenderPass>) -> Result<Eyes, EyeCreationError> {
		let device = queue.device();
		let config = config::get();
		let samples = config.msaa;
		
		let frame_buffer_size = (
			(min_frame_buffer_size.0 as f32 * config.ssaa) as u32,
			(min_frame_buffer_size.1 as f32 * config.ssaa) as u32,
		);
		
		let dimensions = ImageDimensions::Dim2d {
			width: frame_buffer_size.0,
			height: frame_buffer_size.1,
			array_layers: 2,
		};
		
		let side_dimensions = ImageDimensions::Dim2d {
			width: frame_buffer_size.0,
			height: frame_buffer_size.1,
			array_layers: 1,
		};
		
		let resolved_image = AttachmentImage::multisampled_with_usage(device.clone(),
		                                                 dimensions,
		                                                 1,
		                                                 IMAGE_FORMAT,
		                                                 ImageUsage { transfer_source: true,
		                                                              transfer_destination: true,
		                                                              sampled: true,
		                                                              ..ImageUsage::none() })?;
		
		let main_image = if samples > 1 {
			AttachmentImage::multisampled_with_usage(device.clone(),
			                                         dimensions,
			                                         samples,
			                                         IMAGE_FORMAT,
			                                         ImageUsage { color_attachment: true,
			                                                      ..ImageUsage::none() })?
		} else {
			resolved_image.clone()
		};
		
		let side_image = AttachmentImage::multisampled_with_usage(device.clone(),
		                                             side_dimensions,
		                                             1,
		                                             IMAGE_FORMAT,
		                                             ImageUsage { transfer_source: true,
		                                                          transfer_destination: true,
		                                                          sampled: true,
		                                                          ..ImageUsage::none() })?;
		
		let depth_image = AttachmentImage::transient_multisampled(device.clone(), dimensions, samples, DEPTH_FORMAT)?;
		
		let handle_defs = vulkan::Texture {
			image: 0,
			device: device.as_ptr(),
			physical_device: device.physical_device().as_ptr(),
			instance: device.instance().as_ptr(),
			queue: queue.as_ptr(),
			queue_family_index: queue.family().id(),
			width: main_image.dimensions().width(),
			height: main_image.dimensions().height(),
			format: main_image.format() as u32,
			sample_count: main_image.samples(),
		};
		
		let left_texture = Texture {
			handle: Handle::Vulkan(vulkan::Texture {
				image: (*main_image).as_ptr(),
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
		
		let frame_buffer = Framebuffer::with_dimensions(render_pass.clone(),
		                                                [frame_buffer_size.0, frame_buffer_size.1, 1])
			.add(ImageView::new(main_image.clone())?)?
			.add(ImageView::new(depth_image.clone())?)?;
		
		let frame_buffer: Arc<dyn FramebufferAbstract + Send + Sync> = if samples > 1 {
			Arc::new(
				frame_buffer.add(ImageView::new(resolved_image.clone())?)?
				            .build()?
			)
		} else {
			Arc::new(frame_buffer.build()?)
		};
		
		let mut clear_values = vec![ ClearValue::None,
		                             ClearValue::Depth(1.0) ];
		
		if samples > 1 {
			clear_values.push(ClearValue::None)
		}
		
		Ok(Eyes {
			main_image,
			resolved_image,
			side_image,
			depth_image,
			frame_buffer,
			frame_buffer_size,
			textures: (left_texture, right_texture),
			view,
			projection,
			clear_values,
		})
	}
}

#[derive(Debug, Error)]
pub enum EyeCreationError {
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] vulkano::image::ImageCreationError),
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] FramebufferCreationError(#[error(source)] vulkano::render_pass::FramebufferCreationError),
}
