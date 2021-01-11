#![allow(dead_code)]

use std::time::{Instant, Duration};
use err_derive::Error;
use openvr_sys as sys;

mod tracked_camera;

use super::{Camera, CameraCaptureError};
use crate::debug::{debug, get_flag};
use crate::application::VR;
use tracked_camera::{TrackedCamera, FrameType, CameraService};

pub const CAPTURE_INDEX: u32 = 0;

pub struct OpenVR {
	index: sys::TrackedDeviceIndex_t,
	tracked_camera: TrackedCamera,
	service: CameraService,
	last_capture: Instant,
}

impl OpenVR {
	pub fn new(vr: &VR) -> Result<OpenVR, OpenVRCameraError> {
		let index = CAPTURE_INDEX;
		
		let tracked_camera = TrackedCamera::new(&vr.context)?;
		
		println!("{}", debug());
		
		if debug() {
			println!("Has Camera {}", tracked_camera.has_camera(index));
			println!();
			println!("Distorted");
			println!("\t{:?}", tracked_camera.get_camera_frame_size(index, FrameType::Distorted));
			println!("\t\tCamera 0:");
			println!("\t\t\t{:?}", tracked_camera.get_camera_intrinsics(index, 0, FrameType::Distorted));
			println!("\t\t\t{:?}", tracked_camera.get_camera_projection(index, 0, FrameType::Distorted, 0.0, 1.0));
			println!("\t\tCamera 1:");
			println!("\t\t\t{:?}", tracked_camera.get_camera_intrinsics(index, 1, FrameType::Distorted));
			println!("\t\t\t{:?}", tracked_camera.get_camera_projection(index, 1, FrameType::Distorted, 0.0, 1.0));
			println!();
			println!("Undistorted");
			println!("\t{:?}", tracked_camera.get_camera_frame_size(index, FrameType::Undistorted));
			println!("\t\tCamera 0:");
			println!("\t\t\t{:?}", tracked_camera.get_camera_intrinsics(index, 0, FrameType::Undistorted));
			println!("\t\t\t{:?}", tracked_camera.get_camera_projection(index, 0, FrameType::Undistorted, 0.0, 1.0));
			println!("\t\tCamera 1:");
			println!("\t\t\t{:?}", tracked_camera.get_camera_intrinsics(index, 1, FrameType::Undistorted));
			println!("\t\t\t{:?}", tracked_camera.get_camera_projection(index, 1, FrameType::Undistorted, 0.0, 1.0));
			println!();
			println!("MaximumUndistorted");
			println!("\t{:?}", tracked_camera.get_camera_frame_size(index, FrameType::MaximumUndistorted));
			println!("\t\tCamera 0:");
			println!("\t\t\t{:?}", tracked_camera.get_camera_intrinsics(index, 0, FrameType::MaximumUndistorted));
			println!("\t\t\t{:?}", tracked_camera.get_camera_projection(index, 0, FrameType::MaximumUndistorted, 0.0, 1.0));
			println!("\t\tCamera 1:");
			println!("\t\t\t{:?}", tracked_camera.get_camera_intrinsics(index, 1, FrameType::MaximumUndistorted));
			println!("\t\t\t{:?}", tracked_camera.get_camera_projection(index, 1, FrameType::MaximumUndistorted, 0.0, 1.0));
		}
		
		let service = tracked_camera.get_camera_service(index)?;
		
		Ok(OpenVR {
			index,
			tracked_camera,
			service,
			last_capture: Instant::now(),
		})
	}
}

impl Camera for OpenVR {
	fn capture(&mut self) -> Result<&[u8], CameraCaptureError> {
		
		if let Some(cooldown) = Duration::from_millis(16).checked_sub(self.last_capture.elapsed()) {
			std::thread::sleep(cooldown);
		}
		
		let mode: u8 = get_flag("mode").unwrap_or_default();
		
		let mode = match mode {
			0 => FrameType::Distorted,
			1 => FrameType::Undistorted,
			2 => FrameType::MaximumUndistorted,
			_ => unreachable!(),
		};
		
		let ret = self.service.get_frame_buffer(mode)
		                      .map(|fb| fb.buffer.as_slice())
		                      .map_err(|err| match err.code {
			                      sys::EVRTrackedCameraError_VRTrackedCameraError_NoFrameAvailable => CameraCaptureError::Timeout,
			                      _ => CameraCaptureError::Other(err.into()),
		                      });
		
		self.last_capture = Instant::now();
		
		ret
	}
}

#[derive(Debug, Error)]
pub enum OpenVRCameraError {
	#[error(display = "{}", _0)] InitError(#[error(source)] tracked_camera::InitError),
	#[error(display = "{}", _0)] TrackedCameraError(#[error(source)] tracked_camera::TrackedCameraError),
}


