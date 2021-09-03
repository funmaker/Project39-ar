#![allow(dead_code)]

use std::time::{Instant, Duration};
use std::sync::Arc;
use err_derive::Error;
use openvr_sys as sys;
use simba::scalar::SupersetOf;
use openvr::property;
use openvr::system::TrackedPropertyError;

use super::{Camera, CameraCaptureError};
use crate::application::vr::{VR, FrameType, TrackedCameraError, CameraService};
use crate::math::{IVec2, VRSlice, Isometry3, AMat4};
use crate::{debug, config};

pub const CAPTURE_INDEX: u32 = 0;

pub struct OpenVR {
	index: sys::TrackedDeviceIndex_t,
	last_capture: Instant,
	service: CameraService,
	headtocam: Isometry3,
}

impl OpenVR {
	pub fn new(vr: Arc<VR>) -> Result<OpenVR, OpenVRCameraError> {
		let index = CAPTURE_INDEX;
		let headtocam;
		
		{
			let vr = &vr.lock().unwrap();
			let tracked_camera = &vr.tracked_camera;
			
			if debug::debug() {
				println!("Has Camera {}", tracked_camera.has_camera(index));
				println!();
				println!("Distorted");
				println!("\t{:?}", tracked_camera.get_camera_frame_size(index, FrameType::Distorted));
				println!("\t\tCamera 0:");
				println!("\t\t\t{:?}", tracked_camera.get_camera_intrinsics(index, 0, FrameType::Distorted));
				println!("\t\t\t{:?}", tracked_camera.get_camera_projection(index, 0, FrameType::Distorted, 0.01, 100.01));
				println!("\t\tCamera 1:");
				println!("\t\t\t{:?}", tracked_camera.get_camera_intrinsics(index, 1, FrameType::Distorted));
				println!("\t\t\t{:?}", tracked_camera.get_camera_projection(index, 1, FrameType::Distorted, 0.01, 100.01));
				println!();
				println!("Undistorted");
				println!("\t{:?}", tracked_camera.get_camera_frame_size(index, FrameType::Undistorted));
				println!("\t\tCamera 0:");
				println!("\t\t\t{:?}", tracked_camera.get_camera_intrinsics(index, 0, FrameType::Undistorted));
				println!("\t\t\t{:?}", tracked_camera.get_camera_projection(index, 0, FrameType::Undistorted, 0.01, 100.01));
				println!("\t\tCamera 1:");
				println!("\t\t\t{:?}", tracked_camera.get_camera_intrinsics(index, 1, FrameType::Undistorted));
				println!("\t\t\t{:?}", tracked_camera.get_camera_projection(index, 1, FrameType::Undistorted, 0.01, 100.01));
				println!();
				println!("MaximumUndistorted");
				println!("\t{:?}", tracked_camera.get_camera_frame_size(index, FrameType::MaximumUndistorted));
				println!("\t\tCamera 0:");
				println!("\t\t\t{:?}", tracked_camera.get_camera_intrinsics(index, 0, FrameType::MaximumUndistorted));
				println!("\t\t\t{:?}", tracked_camera.get_camera_projection(index, 0, FrameType::MaximumUndistorted, 0.01, 100.01));
				println!("\t\tCamera 1:");
				println!("\t\t\t{:?}", tracked_camera.get_camera_intrinsics(index, 1, FrameType::MaximumUndistorted));
				println!("\t\t\t{:?}", tracked_camera.get_camera_projection(index, 1, FrameType::MaximumUndistorted, 0.01, 100.01));
			}
			
			let frame_size = tracked_camera.get_camera_frame_size(index, FrameType::MaximumUndistorted)?;
			config::rcu(|config|
				config.camera.frame_buffer_size = IVec2::new(frame_size.width as i32, frame_size.height as i32)
			);
			
			let camtohead: Isometry3 = AMat4::from_slice34(&vr.system.matrix34_tracked_device_property(index, property::CameraToHeadTransform_Matrix34)?).to_subset().unwrap();
			headtocam = camtohead.inverse();
		}
		
		let service = CameraService::new(vr, index)?;
		
		Ok(OpenVR {
			index,
			last_capture: Instant::now(),
			service,
			headtocam,
		})
	}
}

impl Camera for OpenVR {
	fn capture(&mut self) -> Result<(&[u8], Option<Isometry3>), CameraCaptureError> {
		let last_capture = self.last_capture;
		self.last_capture = Instant::now();
		
		if let Some(cooldown) = Duration::from_millis(16).checked_sub(last_capture.elapsed()) {
			std::thread::sleep(cooldown);
		}
		
		let mut fb = self.service.get_frame_buffer(FrameType::Distorted)
		                         .map_err(|err| match err.code {
			                         sys::EVRTrackedCameraError_VRTrackedCameraError_NoFrameAvailable => CameraCaptureError::Timeout,
			                         _ => CameraCaptureError::Other(err.into()),
		                         });
		
		// TODO: ???
		if let Ok(ref mut fb) = fb {
			for pos in (0..fb.buffer.len()).step_by(4) {
				let temp = fb.buffer[pos];
				fb.buffer[pos] = fb.buffer[pos + 2];
				fb.buffer[pos + 2] = temp;
			}
		}
		
		let htc = self.headtocam;
		
		fb.map(|fb| (
			fb.buffer.as_slice(),
			AMat4::from_slice34(fb.standing_device_pose.device_to_absolute_tracking())
				.to_subset()
				.map(|pose: Isometry3| pose * htc)
		))
	}
}

#[derive(Debug, Error)]
pub enum OpenVRCameraError {
	#[error(display = "{}", _0)] TrackedCameraError(#[error(source)] TrackedCameraError),
	#[error(display = "{}", _0)] TrackedPropertyError(#[error(source)] TrackedPropertyError),
}


