use openvr::compositor::WaitPoses;
use openvr::{TrackedDevicePose, MAX_TRACKED_DEVICE_COUNT};
use openvr_sys::{TrackedDevicePose_t, HmdMatrix34_t, HmdVector3_t, ETrackedDeviceClass_TrackedDeviceClass_Invalid};

mod vec_future;
pub mod from_args;
mod fps_counter;
mod vulkan;
mod images;
mod id_gen;
mod fence_check;
mod assets;
mod input;
mod rapier;

pub use vec_future::*;
pub use fps_counter::*;
pub use vulkan::*;
pub use images::*;
pub use id_gen::*;
pub use fence_check::*;
pub use assets::*;
pub use input::*;
pub use rapier::*;

pub trait IntoBoxed<T: ?Sized>: 'static {
	fn into(self) -> Box<T>;
}

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
