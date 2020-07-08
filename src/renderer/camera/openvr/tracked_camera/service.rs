use openvr_sys as sys;

use super::{check_err, TrackedCameraError, TrackedCamera};

pub struct CameraService<'a> {
	camera: &'a TrackedCamera,
	handle: sys::TrackedCameraHandle_t,
}

impl<'a> CameraService<'a> {
	pub fn new(camera: &'a TrackedCamera, index: sys::TrackedDeviceIndex_t) -> Result<Self, TrackedCameraError> {
		let mut handle = 0;
		
		check_err(camera.0, unsafe {
			camera.0.AcquireVideoStreamingService.unwrap()(index,
			                                               &mut handle)
		})?;
		
		Ok(CameraService {
			camera,
			handle,
		})
	}
}

impl<'a> Drop for CameraService<'a> {
	fn drop(&mut self) { unsafe {
		check_err(self.camera.0, self.camera.0.ReleaseVideoStreamingService.unwrap()(self.handle))
			.expect("Unable to release video streaming service");
	}}
}

