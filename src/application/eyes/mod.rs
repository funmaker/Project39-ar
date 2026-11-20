use std::cell::Cell;
use std::sync::Arc;
use anyhow::Result;
use thiserror::Error;
use openvr::compositor::Texture;
use openvr::compositor::texture::{ColorSpace, Handle, vulkan};
use vulkano::{command_buffer, sync};
use vulkano::command_buffer::{CopyImageInfo, ImageCopy};
use vulkano::format::ClearValue;
use vulkano::image::{Image, ImageCreateInfo, ImageSubresourceLayers, ImageUsage};
use vulkano::memory::allocator::AllocationCreateInfo;
use vulkano::sync::GpuFuture;
use simba::scalar::SupersetOf;

pub mod camera;
mod background;
mod pipeline;
pub mod dump;

use crate::debug;
use crate::config::{self, NovrConfig};
use crate::math::{AMat4, Isometry3, Mat4, Perspective3, PI, PMat4, projective_clip, SubsetOfLossy, Vec4, VRSlice};
use crate::renderer::{IMAGE_FORMAT, RenderContext, Renderer, RenderTarget, RenderTargetContext};
use crate::utils::{default_tracked_pose, FramebufferBundle, OpenVRPtr};
use super::VR;
use super::vr::VRInner;
use super::vr::FrameType;
use background::Background;
use camera::Camera;
use dump::{EyesDump, OpenVRDump, ProjectionDump};


pub struct Eyes {
	fb: FramebufferBundle,
	side_image: Arc<Image>, // TODO: https://github.com/ValveSoftware/openvr/issues/663
	textures: (Texture, Texture),
	view: (AMat4, AMat4),
	projection: (PMat4, PMat4),
	raw_projection: (Vec4, Vec4),
	vr: Option<Arc<VR>>,
	background: Option<Background>,
	hmd_pose: [[f32; 4]; 3],
}


impl Eyes {
	pub fn new_novr(config: &NovrConfig, camera: Option<Box<dyn Camera>>, renderer: &mut Renderer) -> Result<Eyes> {
		let min_framebuffer_size = (config.frame_buffer_width, config.frame_buffer_height);
		let aspect = config.frame_buffer_width as f32 / config.frame_buffer_height as f32;
		let fovx = config.fov / 180.0 * PI;
		let fovy = ((fovx / 2.0).tan() / aspect).atan() * 2.0;
		
		let view = AMat4::identity();
		let projection = projective_clip() * Perspective3::new(aspect, fovy, 0.1, 100.0).as_projective();
		let raw = vector!((fovx / 2.0).tan(), (fovx / 2.0).tan(), (fovy / 2.0).tan(), (fovy / 2.0).tan());
		
		Self::new(min_framebuffer_size, (view, view), (projection, projection), (raw, raw), [0.0; 2], None, camera, renderer)
	}
	
	pub fn new_vr(vr: Arc<VR>, camera: Option<Box<dyn Camera>>, renderer: &mut Renderer) -> Result<Eyes> {
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
		
		let quirk_shift = Self::steamvr_quirk_shift(&vr_lock);

		drop(vr_lock);

		Self::new(min_framebuffer_size, (view_left, view_right), (proj_left, proj_right), (raw_left, raw_right), quirk_shift, Some(vr), camera, renderer)
	}

	/// SteamVR samples the right camera's undistortion map (frame size / 4, built from the MaximumUndistorted
	/// projection) half a texel off: it reads the undistorted u of `u - 1 / (map_width - 1)`.
	/// Returns that shift in camera space x/z for each camera.
	fn steamvr_quirk_shift(vr: &VRInner) -> [f32; 2] {
		let hmd = openvr::tracked_device_index::HMD;
		let config = config::get();
		let map_width = config.camera.frame_buffer_size.x as f32 / 4.0;

		match vr.tracked_camera.get_camera_projection(hmd, 1, FrameType::MaximumUndistorted, 0.1, 1000.0) {
			Ok(projection) if config.camera.right.offset.x > 0 && map_width > 1.0 => [0.0, 1.0 / (map_width - 1.0) / projection[0][0]],
			_ => [0.0; 2],
		}
	}
	
	pub fn new(min_framebuffer_size: (u32, u32), view: (AMat4, AMat4), projection: (PMat4, PMat4), raw_projection: (Vec4, Vec4), quirk_shift: [f32; 2], vr: Option<Arc<VR>>, camera: Option<Box<dyn Camera>>, renderer: &mut Renderer) -> Result<Eyes> {
		let background = camera.map(|camera| Background::new(camera, raw_projection, view, quirk_shift, vr.clone(), renderer))
		                       .transpose()?;
		
		let fb = renderer.create_framebuffer(min_framebuffer_size)?;
		
		let dimensions = fb.framebuffer.extent();
		
		let side_image = Image::new(renderer.memory_allocator.clone(),
		                            ImageCreateInfo {
			                            extent: [dimensions[0], dimensions[1], 1],
			                            format: IMAGE_FORMAT,
			                            usage: ImageUsage::TRANSFER_SRC
			                                   | ImageUsage::TRANSFER_DST
			                                   | ImageUsage::SAMPLED,
			                            ..ImageCreateInfo::default()
		                            },
		                            AllocationCreateInfo::default())?;
		
		let handle_defs = vulkan::Texture {
			image: 0,
			device: renderer.device.as_ptr(),
			physical_device: renderer.device.physical_device().as_ptr(),
			instance: renderer.instance.as_ptr(),
			queue: renderer.queue.as_ptr(),
			queue_family_index: renderer.queue.queue_family_index(),
			width: fb.main_image.extent()[0],
			height: fb.main_image.extent()[1],
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
	
	pub fn load_background(&mut self, camera_pos: Isometry3, renderer: &mut Renderer) -> Result<()> {
		let head_pose = self.tracking_head_pose(camera_pos);

		if let Some(background) = &mut self.background {
			renderer.try_enqueue(renderer.load_queue.clone(), |future| background.load_camera(head_pose, future))?;
		}

		Ok(())
	}

	/// Head pose in tracking space: HMD pose in standing space in VR (independent of the VR root), POV in novr.
	fn tracking_head_pose(&self, camera_pos: Isometry3) -> Isometry3 {
		if self.vr.is_some() {
			AMat4::from_slice34(&self.hmd_pose).to_subset().unwrap_or(camera_pos)
		} else {
			camera_pos
		}
	}
	
	pub fn set_hmd_pose(&mut self, hmd_pose: [[f32; 4]; 3]) {
		self.hmd_pose = hmd_pose;
	}
	
	pub fn framebuffer_size(&self) -> (u32, u32) {
		self.fb.extent()
	}
	
	/// Stereo (2 layer) image submitted to the HMD.
	pub fn framebuffer_image(&self) -> &Arc<Image> {
		&self.fb.main_image
	}
	
	pub fn camera_image(&self) -> Option<&Arc<Image>> {
		self.background.as_ref().map(Background::camera_image)
	}

	pub fn steamvr_quirks(&self) -> Option<&Cell<bool>> {
		self.background.as_ref().map(Background::steamvr_quirks)
	}

	/// 3D passthrough toggle, available in VR only.
	pub fn depth_passthrough(&self) -> Option<&Cell<bool>> {
		self.background.as_ref().filter(|_| self.vr.is_some()).map(Background::depth_passthrough)
	}

	/// All data used for the passthrough projection of the last rendered frame.
	pub fn projection_dump(&self) -> ProjectionDump {
		let config = config::get();
		let fb_size = self.framebuffer_size();
		let camera_size = self.camera_image().map_or([0, 0], |image| [image.extent()[0], image.extent()[1]]);
		
		let openvr = self.vr.as_ref().map(|vr| {
			let vr = vr.lock().unwrap();
			let hmd = openvr::tracked_device_index::HMD;
			let raw = |eye| {
				let raw = vr.system.projection_raw(eye);
				[raw.left, raw.right, raw.top, raw.bottom]
			};
			
			OpenVRDump {
				camera_to_head_row_major: vr.system.matrix34_tracked_device_property(hmd, openvr::property::CameraToHeadTransform_Matrix34).ok(),
				distorted_frame_size: vr.tracked_camera.get_camera_frame_size(hmd, FrameType::Distorted).ok().map(|size| [size.width, size.height]),
				intrinsics: (0..2).filter_map(|camera| vr.tracked_camera.get_camera_intrinsics(hmd, camera, FrameType::MaximumUndistorted).ok())
				                            .map(|i| [i.focal_length[0], i.focal_length[1], i.center[0], i.center[1]])
				                            .collect(),
				eye_to_head_row_major: [vr.system.eye_to_head_transform(openvr::Eye::Left), vr.system.eye_to_head_transform(openvr::Eye::Right)],
				projection_raw: [raw(openvr::Eye::Left), raw(openvr::Eye::Right)],
			}
		});
		
		ProjectionDump {
			framebuffer_size: [fb_size.0, fb_size.1],
			camera_frame_size: camera_size,
			camera_frame_buffer_size: config.camera.frame_buffer_size.into(),
			hmd_pose_row_major: self.hmd_pose,
			eyes: EyesDump {
				view: [dump::mat4(&self.view.0.to_homogeneous()), dump::mat4(&self.view.1.to_homogeneous())],
				projection: [dump::mat4(&self.projection.0.to_homogeneous()), dump::mat4(&self.projection.1.to_homogeneous())],
				raw_projection: [self.raw_projection.0.into(), self.raw_projection.1.into()],
			},
			background: self.background.as_ref().map(Background::dump),
			openvr,
			camera_config: config.camera.clone(),
		}
	}
}

impl RenderTarget for Eyes {
	fn create_context(&mut self, camera_pos: Isometry3) -> Result<Option<RenderTargetContext>> {
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
	
	fn last_frame(&self) -> &Arc<Image> {
		&self.fb.main_image
	}
	
	fn early_render(&mut self, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<()> {
		let head_pose = self.tracking_head_pose(context.camera_pos);

		if let Some(background) = &mut self.background {
			background.render(head_pose, context.builder)?;
		}
		
		Ok(())
	}
	
	fn after_render(&mut self, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<()> {
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
	
	fn after_execute(&mut self, _renderer: &mut Renderer) -> Result<()> {
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

