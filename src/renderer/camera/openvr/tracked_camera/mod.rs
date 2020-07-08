use openvr::Context;
use openvr_sys as sys;

mod error;
pub use error::*;
mod utils;
pub use utils::*;
mod service;
pub use service::*;

pub struct TrackedCamera(FnTable);

impl TrackedCamera {
	pub fn new(_context: &Context) -> Result<TrackedCamera, InitError> {
		let fn_tab: FnTable = unsafe { &*load(sys::IVRTrackedCamera_Version)? };
		
		Ok(TrackedCamera(fn_tab))
	}
	
	pub fn has_camera(&self, index: sys::TrackedDeviceIndex_t) -> bool {
		let mut out = false;
		
		unsafe { self.0.HasCamera.unwrap()(index, &mut out); }
		
		out
	}
	
	pub fn get_camera_frame_size(&self, index: sys::TrackedDeviceIndex_t, frame_type: FrameType) -> Result<FrameSize, TrackedCameraError> {
		let mut out = FrameSize::default();
		
		check_err(self.0, unsafe {
			self.0.GetCameraFrameSize.unwrap()(index,
			                                   frame_type.into(),
			                                   &mut out.width,
			                                   &mut out.height,
			                                   &mut out.frame_buffer_size)
		})?;
		
		Ok(out)
	}
	
	pub fn get_camera_intrinsics(&self, index: sys::TrackedDeviceIndex_t, camera_index: u32, frame_type: FrameType) -> Result<Intrinsics, TrackedCameraError> {
		let mut out = Intrinsics::default();
		
		check_err(self.0, unsafe {
			self.0.GetCameraIntrinsics.unwrap()(index,
			                                    camera_index,
			                                    frame_type.into(),
			                                    &mut out.focal_length as *mut _ as *mut sys::HmdVector2_t,
			                                    &mut out.center as *mut _ as *mut sys::HmdVector2_t)
		})?;
		
		Ok(out)
	}
	
	pub fn get_camera_projection(&self, index: sys::TrackedDeviceIndex_t, camera_index: u32, frame_type: FrameType, z_near: f32, z_far: f32) -> Result<[[f32; 4]; 4], TrackedCameraError> {
		let mut out = [[0.0; 4]; 4];
		
		check_err(self.0, unsafe {
			self.0.GetCameraProjection.unwrap()(index,
			                                    camera_index,
			                                    frame_type.into(),
			                                    z_near,
			                                    z_far,
			                                    &mut out as *mut _ as *mut sys::HmdMatrix44_t)
		})?;
		
		Ok(out)
	}
	
	pub fn get_camera_service(&self, index: sys::TrackedDeviceIndex_t) -> Result<CameraService, TrackedCameraError> {
		CameraService::new(&self, index)
	}
}

fn load<T>(suffix: &[u8]) -> Result<*const T, InitError> {
	let mut magic = Vec::from(b"FnTable:".as_ref());
	magic.extend(suffix);
	let mut error = sys::EVRInitError_VRInitError_None;
	let result = unsafe { sys::VR_GetGenericInterface(magic.as_ptr() as *const i8, &mut error) };
	if error != sys::EVRInitError_VRInitError_None {
		return Err(InitError(
			sys::EVRInitError_VRInitError_Init_InterfaceNotFound,
		));
	}
	Ok(result as *const T)
}
