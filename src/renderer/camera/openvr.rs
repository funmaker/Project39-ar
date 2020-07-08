#![allow(dead_code)]

use err_derive::Error;
use openvr_sys as sys;
use openvr::Context;

use super::{ CAPTURE_WIDTH, CAPTURE_HEIGHT, CAPTURE_FPS, Camera, CaptureError };

mod tracked_camera;
use tracked_camera::{TrackedCamera, FrameType};

pub const CAPTURE_INDEX: u32 = 0;

pub struct OpenVR {
	index: sys::TrackedDeviceIndex_t,
	tracked_camera: TrackedCamera,
}

impl OpenVR {
	pub fn new(context: &Context) -> Result<OpenVR, OpenVRCameraError> {
		let index = CAPTURE_INDEX;
		
		let tracked_camera = TrackedCamera::new(context)?;
		
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
		panic!();
		
		{
			let _service = tracked_camera.get_camera_service(index)?;
		}
		
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
	#[error(display = "{}", _0)] TrackedCameraError(#[error(source)] tracked_camera::TrackedCameraError),
}


