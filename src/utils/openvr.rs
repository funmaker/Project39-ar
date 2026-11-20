use openvr::{TrackedDevicePose, MAX_TRACKED_DEVICE_COUNT};
use openvr::compositor::WaitPoses;
use openvr_sys::{TrackedDevicePose_t, HmdMatrix34_t, HmdVector3_t, ETrackedDeviceClass_TrackedDeviceClass_Invalid};

use crate::application::Application;

pub fn default_tracked_pose() -> TrackedDevicePose {
	TrackedDevicePose::from(TrackedDevicePose_t {
		mDeviceToAbsoluteTracking: HmdMatrix34_t { m: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]] },
		vVelocity: HmdVector3_t { v: [0.0, 0.0, 0.0] },
		vAngularVelocity: HmdVector3_t { v: [0.0, 0.0, 0.0] },
		eTrackingResult: ETrackedDeviceClass_TrackedDeviceClass_Invalid,
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

pub fn dump_camera_ss(application: &Application) {
	
}
