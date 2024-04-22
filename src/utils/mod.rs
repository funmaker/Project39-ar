use openvr::{TrackedDevicePose, MAX_TRACKED_DEVICE_COUNT};
use openvr::compositor::WaitPoses;
use openvr_sys::{TrackedDevicePose_t, HmdMatrix34_t, HmdVector3_t, ETrackedDeviceClass_TrackedDeviceClass_Invalid};

pub mod from_args;
mod fence_check;
mod fps_counter;
mod gui;
mod id_gen;
mod images;
mod index_buffer;
mod input;
mod mut_mark;
mod pattern;
mod rapier;
mod vulkan;
mod ref_cell_iter;

pub use fence_check::*;
pub use fps_counter::*;
pub use gui::*;
pub use id_gen::*;
pub use images::*;
pub use index_buffer::*;
pub use input::*;
pub use mut_mark::*;
pub use pattern::*;
pub use rapier::*;
pub use vulkan::*;
pub use ref_cell_iter::*;


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

macro_rules! collection {
    // map-like
    ($($k:expr => $v:expr),* $(,)?) => {{
        use std::iter::{Iterator, IntoIterator};
        Iterator::collect(IntoIterator::into_iter([$(($k, $v),)*]))
    }};
    // set-like
    ($($v:expr),* $(,)?) => {{
        use std::iter::{Iterator, IntoIterator};
        Iterator::collect(IntoIterator::into_iter([$($v,)*]))
    }};
}
