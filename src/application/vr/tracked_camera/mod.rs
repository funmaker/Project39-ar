use ::openvr_sys as sys;
use openvr::{Context, TrackedDeviceIndex};

mod error;
mod utils;

pub use error::*;
pub use utils::*;


#[derive(Copy, Clone)]
pub struct TrackedCamera(FnTable);

impl TrackedCamera {
	pub fn new(_context: &Context) -> Result<TrackedCamera, InitError> {
		let fn_tab: FnTable = unsafe { &*load(sys::IVRTrackedCamera_Version)? };
		
		Ok(TrackedCamera(fn_tab))
	}
	
	pub fn has_camera(&self, index: TrackedDeviceIndex) -> bool {
		let mut out = false;
		
		unsafe { self.0.HasCamera.unwrap()(index, &mut out); }
		
		out
	}
	
	pub fn get_camera_frame_size(&self, index: TrackedDeviceIndex, frame_type: FrameType) -> Result<FrameSize, TrackedCameraError> {
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
	
	pub fn get_camera_intrinsics(&self, index: TrackedDeviceIndex, camera_index: u32, frame_type: FrameType) -> Result<Intrinsics, TrackedCameraError> {
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
	
	pub fn get_camera_projection(&self, index: TrackedDeviceIndex, camera_index: u32, frame_type: FrameType, z_near: f32, z_far: f32) -> Result<[[f32; 4]; 4], TrackedCameraError> {
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
	
	pub unsafe fn acquire_video_streaming_service(&self, index: TrackedDeviceIndex) -> Result<TrackedCameraHandle, TrackedCameraError> {
		let mut out = 0;
		
		check_err(self.0,
			self.0.AcquireVideoStreamingService.unwrap()(index,
			                                             &mut out)
		)?;
		
		Ok(out)
	}
	
	pub unsafe fn release_video_streaming_service(&self, handle: TrackedCameraHandle) -> Result<(), TrackedCameraError> {
		check_err(self.0,
			self.0.ReleaseVideoStreamingService.unwrap()(handle)
		)?;
		
		Ok(())
	}
	
	pub unsafe fn get_video_stream_frame_buffer(&self, handle: TrackedCameraHandle, frame_type: FrameType, buffer: &mut [u8]) -> Result<CameraVideoStreamFrameHeader, TrackedCameraError> {
		let mut header = CameraVideoStreamFrameHeader {
			eFrameType: frame_type.into(),
			nWidth: 0,
			nHeight: 0,
			nBytesPerPixel: 0,
			nFrameSequence: 0,
			standingTrackedDevicePose: sys::TrackedDevicePose_t {
				mDeviceToAbsoluteTracking: sys::HmdMatrix34_t { m: [[0.0; 4]; 3] },
				vVelocity: sys::HmdVector3_t { v: [0.0, 0.0, 0.0] },
				vAngularVelocity: sys::HmdVector3_t { v: [0.0, 0.0, 0.0] },
				eTrackingResult: 0,
				bPoseIsValid: false,
				bDeviceIsConnected: false
			},
			ulFrameExposureTime: 0
		};
		
		check_err(self.0,
			self.0.GetVideoStreamFrameBuffer.unwrap()(handle,
			                                          frame_type.into(),
			                                          buffer.as_mut_ptr() as *mut _,
			                                          buffer.len() as u32,
			                                          &mut header,
			                                          std::mem::size_of::<CameraVideoStreamFrameHeader>() as u32)
		)?;
		
		Ok(header)
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
