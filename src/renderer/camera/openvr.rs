#![allow(dead_code)]

use std::time::{Instant, Duration};
use std::sync::Arc;
use err_derive::Error;
use openvr_sys as sys;

use super::{Camera, CameraCaptureError};
use crate::application::vr::{VR, FrameType, TrackedCameraError, CameraService};
use crate::math::IVec2;
use crate::{debug, config};

pub const CAPTURE_INDEX: u32 = 0;

pub struct OpenVR {
	index: sys::TrackedDeviceIndex_t,
	last_capture: Instant,
	service: CameraService,
}

impl OpenVR {
	pub fn new(vr: Arc<VR>) -> Result<OpenVR, OpenVRCameraError> {
		let index = CAPTURE_INDEX;
		
		{
			let tracked_camera = &vr.lock().unwrap().tracked_camera;
			
			if debug::debug() {
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
			
			let frame_size = tracked_camera.get_camera_frame_size(index, FrameType::MaximumUndistorted)?;
			config::rcu(|config|
				config.camera.frame_buffer_size = IVec2::new(frame_size.width as i32, frame_size.height as i32)
			);
		}
		
		let service = CameraService::new(vr, index)?;
		
		Ok(OpenVR {
			index,
			last_capture: Instant::now(),
			service,
		})
	}
}

impl Camera for OpenVR {
	fn capture(&mut self) -> Result<&[u8], CameraCaptureError> {
		
		if let Some(cooldown) = Duration::from_millis(16).checked_sub(self.last_capture.elapsed()) {
			std::thread::sleep(cooldown);
		}
		
		let mode: u8 = debug::get_flag("mode").unwrap_or_default();
		
		let mode = match mode {
			0 => FrameType::Distorted,
			1 => FrameType::Undistorted,
			2 => FrameType::MaximumUndistorted,
			_ => unreachable!(),
		};
		
		let mut ret;
		
		ret = self.service.get_frame_buffer(mode)
		                  .map(|fb| fb.buffer.as_mut_slice())
		                  .map_err(|err| match err.code {
			                  sys::EVRTrackedCameraError_VRTrackedCameraError_NoFrameAvailable => CameraCaptureError::Timeout,
			                  _ => CameraCaptureError::Other(err.into()),
		                  });
		
		// TODO: probably can be ignored later
		if let Ok(ref mut slice) = ret {
			for pos in (3..slice.len()).step_by(4) {
				slice[pos] = 255;
			}
		}
		
		self.last_capture = Instant::now();
		
		ret.map(|x| &*x)
	}
}

#[derive(Debug, Error)]
pub enum OpenVRCameraError {
	#[error(display = "{}", _0)] TrackedCameraError(#[error(source)] TrackedCameraError),
}


