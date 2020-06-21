use std::{fmt, error, ops};
use std::ffi::CStr;
use std::error::Error;

use openvr::Context;
use openvr_sys as sys;

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

pub struct InitError(sys::EVRInitError);

impl fmt::Debug for InitError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let msg = unsafe { CStr::from_ptr(sys::VR_GetVRInitErrorAsSymbol(self.0)) };
		f.pad(
			msg.to_str()
			   .expect("OpenVR init error symbol was not valid UTF-8"),
		)
	}
}

impl error::Error for InitError {
	fn description(&self) -> &str {
		let msg = unsafe { CStr::from_ptr(sys::VR_GetVRInitErrorAsEnglishDescription(self.0)) };
		msg.to_str()
		   .expect("OpenVR init error description was not valid UTF-8")
	}
}

impl fmt::Display for InitError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.pad(error::Error::description(self))
	}
}
