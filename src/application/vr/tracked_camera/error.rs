use std::{error, fmt};
use std::ffi::CStr;
use ::openvr_sys as sys;

use super::FnTable;


pub struct InitError(pub sys::EVRInitError);

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

pub struct TrackedCameraError {
	pub code: sys::EVRTrackedCameraError,
	pub name: String,
}

impl error::Error for TrackedCameraError {}

impl fmt::Debug for TrackedCameraError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.pad(&format!("{}({})", self.name, self.code))
	}
}

impl fmt::Display for TrackedCameraError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.pad(&self.name)
	}
}

pub fn check_err(fn_tab: FnTable, code: sys::EVRTrackedCameraError) -> Result<(), TrackedCameraError> {
	if code == sys::EVRTrackedCameraError_VRTrackedCameraError_None {
		Ok(())
	} else {
		let name = fn_tab.GetCameraErrorNameFromEnum
		                 .map(|f| unsafe { f(code) })
		                 .map(|msg| unsafe { CStr::from_ptr(msg) })
		                 .map(CStr::to_str)
		                 .map(Result::ok)
		                 .flatten()
		                 .unwrap_or("VRTrackedCameraError_UnknownError")
		                 .into();
		
		Err(TrackedCameraError{ code, name })
	}
}
