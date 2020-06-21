#![allow(dead_code)]

use err_derive::Error;
use openvr_sys as sys;
use openvr::Context;

use super::{ CAPTURE_WIDTH, CAPTURE_HEIGHT, CAPTURE_FPS, Camera, CaptureError };

mod tracked_camera;
use tracked_camera::TrackedCamera;

pub const CAPTURE_INDEX: u32 = 0;

pub struct OpenVR {
	index: sys::TrackedDeviceIndex_t,
	tracked_camera: TrackedCamera,
}

impl OpenVR {
	pub fn new(context: &Context) -> Result<OpenVR, OpenVRCameraError> {
		let index = CAPTURE_INDEX;
		
		let tracked_camera = TrackedCamera::new(context)?;
		
		panic!("has {}", tracked_camera.has_camera(index));
		
		Ok(OpenVR {
			index,
			tracked_camera,
		})
	}
}

impl Camera for OpenVR {
	fn capture(&mut self) -> Result<&[u8], CaptureError> {
		unimplemented!()
	}
}

#[derive(Debug, Error)]
pub enum OpenVRCameraError {
	#[error(display = "{}", _0)] APIError(#[error(source)] escapi::Error),
	#[error(display = "{}", _0)] InitError(#[error(source)] tracked_camera::InitError),
}


