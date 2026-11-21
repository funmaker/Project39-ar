use std::{error, fmt};
use std::ffi::CStr;
use openvr::{TrackedDevicePose, MAX_TRACKED_DEVICE_COUNT};
use openvr::compositor::WaitPoses;
use openvr_sys as sys;

pub fn default_tracked_pose() -> TrackedDevicePose {
	TrackedDevicePose::from(sys::TrackedDevicePose_t {
		mDeviceToAbsoluteTracking: sys::HmdMatrix34_t { m: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]] },
		vVelocity: sys::HmdVector3_t { v: [0.0, 0.0, 0.0] },
		vAngularVelocity: sys::HmdVector3_t { v: [0.0, 0.0, 0.0] },
		eTrackingResult: sys::ETrackedDeviceClass_TrackedDeviceClass_Invalid,
		bPoseIsValid: false,
		bDeviceIsConnected: false
	})
}

pub fn default_wait_poses() -> WaitPoses {
	WaitPoses {
		render: [default_tracked_pose(); MAX_TRACKED_DEVICE_COUNT],
		game: [default_tracked_pose(); MAX_TRACKED_DEVICE_COUNT],
	}
}

/// Loads OpenVR's C function table of interface `version` (e.g. `openvr_sys::IVRSystem_Version`).
pub fn load_fn_table<T>(version: &[u8]) -> Result<*const T, OpenVRFnTableError> {
	let mut magic = Vec::from(b"FnTable:".as_ref());
	magic.extend(version);
	let mut error = sys::EVRInitError_VRInitError_None;
	let fn_table = unsafe { sys::VR_GetGenericInterface(magic.as_ptr() as *const _, &mut error) as *const T };
	if error != sys::EVRInitError_VRInitError_None || fn_table.is_null() {
		return Err(OpenVRFnTableError(error));
	}

	Ok(fn_table)
}

pub struct OpenVRFnTableError(pub sys::EVRInitError);

impl error::Error for OpenVRFnTableError {}

impl fmt::Debug for OpenVRFnTableError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let msg = unsafe { CStr::from_ptr(sys::VR_GetVRInitErrorAsSymbol(self.0)) };
		f.pad(
			msg.to_str()
			   .expect("OpenVR init error symbol was not valid UTF-8"),
		)
	}
}

impl fmt::Display for OpenVRFnTableError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let msg = unsafe { CStr::from_ptr(sys::VR_GetVRInitErrorAsEnglishDescription(self.0)) };
		f.pad(
			msg.to_str()
			   .expect("OpenVR init error description was not valid UTF-8")
		)
	}
}
