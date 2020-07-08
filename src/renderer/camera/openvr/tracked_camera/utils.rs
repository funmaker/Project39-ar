use openvr_sys as sys;

pub type FnTable = &'static sys::VR_IVRTrackedCamera_FnTable;

#[derive(Default, Debug)]
pub struct FrameSize {
	pub width: u32,
	pub height: u32,
	pub frame_buffer_size: u32,
}

#[derive(Default, Debug)]
pub struct Intrinsics {
	pub width: u32,
	pub focal_length: [f32; 2],
	pub center: [f32; 2],
}

#[derive(Default, Debug)]
pub struct Projection {
	pub z_near: f32,
	pub z_far: f32,
	pub projection: [[f32; 4]; 4],
}

#[derive(Debug, Clone, Copy)]
pub enum FrameType {
	Distorted = 0,
	Undistorted = 1,
	MaximumUndistorted = 2,
}

impl Into<sys::EVRTrackedCameraError> for FrameType { fn into(self) -> sys::EVRTrackedCameraError { self as sys::EVRTrackedCameraError } }
