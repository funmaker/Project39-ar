use ::openvr_sys as sys;


pub type FnTable = &'static sys::VR_IVRTrackedCamera_FnTable;
pub type TrackedCameraHandle = openvr_sys::TrackedCameraHandle_t;
pub type CameraVideoStreamFrameHeader = openvr_sys::CameraVideoStreamFrameHeader_t;

#[derive(Default, Debug)]
pub struct FrameSize {
	pub width: u32,
	pub height: u32,
	pub frame_buffer_size: u32,
}

#[derive(Default, Debug)]
pub struct Intrinsics {
	pub focal_length: [f32; 2],
	pub center: [f32; 2],
}

#[derive(Default, Debug)]
pub struct Projection {
	pub z_near: f32,
	pub z_far: f32,
	pub projection: [[f32; 4]; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
	Distorted = sys::EVRTrackedCameraFrameType_VRTrackedCameraFrameType_Distorted as isize,
	Undistorted = sys::EVRTrackedCameraFrameType_VRTrackedCameraFrameType_Undistorted as isize,
	MaximumUndistorted = sys::EVRTrackedCameraFrameType_VRTrackedCameraFrameType_MaximumUndistorted as isize,
}

impl Into<sys::EVRTrackedCameraFrameType> for FrameType { fn into(self) -> sys::EVRTrackedCameraFrameType { self as sys::EVRTrackedCameraFrameType } }
impl Into<FrameType> for sys::EVRTrackedCameraFrameType { fn into(self) -> FrameType {
	match self {
		sys::EVRTrackedCameraFrameType_VRTrackedCameraFrameType_Distorted => FrameType::Distorted,
		sys::EVRTrackedCameraFrameType_VRTrackedCameraFrameType_Undistorted => FrameType::Undistorted,
		sys::EVRTrackedCameraFrameType_VRTrackedCameraFrameType_MaximumUndistorted => FrameType::MaximumUndistorted,
		_ => panic!("Unknown TrackedCameraFrameType = {}", self),
	}
} }
