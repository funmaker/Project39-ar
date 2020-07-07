use std::{fmt, error};
use std::ffi::CStr;
use std::error::Error;

use openvr::Context;
use openvr_sys as sys;
use std::fmt::Debug;
use std::convert::TryInto;

pub struct TrackedCamera(&'static sys::VR_IVRTrackedCamera_FnTable);

impl TrackedCamera {
	pub fn new(_context: &Context) -> Result<TrackedCamera, InitError> {
		let fn_tab: &'static sys::VR_IVRTrackedCamera_FnTable = unsafe { &*load(sys::IVRTrackedCamera_Version)? };
		
		Ok(TrackedCamera(fn_tab))
	}
	
	pub fn has_camera(&self, index: sys::TrackedDeviceIndex_t) -> bool {
		let mut out = false;
		
		unsafe { self.0.HasCamera.unwrap()(index, &mut out); }
		
		out
	}
	
	pub fn get_camera_frame_size(&self, index: sys::TrackedDeviceIndex_t, frame_type: FrameType) -> Result<FrameSize, TrackedCameraError> {
		let mut out = FrameSize::default();
		
		if let Some(error) = self.wrap_err(unsafe {
			self.0.GetCameraFrameSize.unwrap()(index,
			                                   frame_type.into(),
			                                   &mut out.width,
			                                   &mut out.height,
			                                   &mut out.frame_buffer_size)
		}) {
			return Err(error);
		}
		
		Ok(out)
	}
	
	fn wrap_err(&self, error: sys::EVRTrackedCameraError) -> Option<TrackedCameraError> {
		if error == sys::EVRTrackedCameraError_VRTrackedCameraError_None {
			None
		} else {
			Some(TrackedCameraError(error, self.0))
		}
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

// Frame Size

#[derive(Default, Debug)]
pub struct FrameSize {
	pub width: u32,
	pub height: u32,
	pub frame_buffer_size: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum FrameType {
	Distorted = 0,
	Undistorted = 1,
	MaximumUndistorted = 2,
}

impl Into<sys::EVRTrackedCameraError> for FrameType { fn into(self) -> sys::EVRTrackedCameraError { self as sys::EVRTrackedCameraError } }

// ERRORS

pub struct InitError(sys::EVRInitError);

impl error::Error for InitError {}

impl fmt::Debug for InitError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let msg = unsafe { CStr::from_ptr(sys::VR_GetVRInitErrorAsSymbol(self.0)) };
		f.pad(
			msg.to_str()
			   .expect("OpenVR init error symbol was not valid UTF-8"),
		)
	}
}

impl fmt::Display for InitError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let msg = unsafe { CStr::from_ptr(sys::VR_GetVRInitErrorAsEnglishDescription(self.0)) };
		f.pad(
			msg.to_str()
			   .expect("OpenVR init error description was not valid UTF-8")
		)
	}
}

pub struct TrackedCameraError(sys::EVRTrackedCameraError, &'static sys::VR_IVRTrackedCamera_FnTable);

impl Error for TrackedCameraError {}

impl fmt::Debug for TrackedCameraError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { unsafe {
		f.pad(self.1.GetCameraErrorNameFromEnum.map(|f| f(self.0))
		                                       .map(|msg| CStr::from_ptr(msg))
		                                       .map(CStr::to_str)
		                                       .map(Result::ok)
		                                       .flatten()
		                                       .unwrap_or("Unknown Tracked Camera Error"))
	}}
}

impl fmt::Display for TrackedCameraError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		fmt::Debug::fmt(&self, f)
	}
}
