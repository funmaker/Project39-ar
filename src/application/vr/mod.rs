use std::ops::Deref;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Result;
use thiserror::Error;
use openvr::{Context, System, Compositor, RenderModels, TrackedDeviceProperty, tracked_device_index};
use openvr_sys as sys;

mod camera_service;
mod depth_mesh;
mod tracked_camera;

pub use camera_service::CameraService;
pub use depth_mesh::{DepthMesh, DepthMeshFrame};
pub use tracked_camera::{TrackedCamera, FrameType};

use crate::config::{self, CameraAPI};
use crate::math::Vec3;
use crate::utils::load_fn_table;


static VR_CREATED: AtomicBool = AtomicBool::new(false);

pub struct VRInner {
	pub context: Context,
	pub system: System,
	pub compositor: Compositor,
	pub render_models: RenderModels,
	pub tracked_camera: TrackedCamera,
}

impl !Sync for VRInner {}

impl Drop for VRInner {
	fn drop(&mut self) {
		// Context has to be shutdown before dropping graphical API
		unsafe { self.context.shutdown(); }
		VR_CREATED.store(false, Ordering::SeqCst);
	}
}

pub struct VR(Mutex<VRInner>);

impl VR {
	pub fn new() -> Result<VR> {
		if VR_CREATED.swap(true, Ordering::SeqCst) {
			return Err(VRError::AlreadyInitialized.into());
		}
		
		let context = unsafe { openvr::init(openvr::ApplicationType::Scene) }?;
		
		let system = context.system()?;
		let compositor = context.compositor()?;
		let render_models = context.render_models()?;
		let tracked_camera = TrackedCamera::new(&context)?;
		
		if config::get().camera.driver == CameraAPI::OpenVR {
			if let Err(err) = update_camera_config(&system, &tracked_camera) {
				eprintln!("Unable to read HMD camera calibration from OpenVR, using config values: {err}");
			}
		}
		
		Ok(VR(Mutex::new(VRInner{
			context,
			system,
			compositor,
			render_models,
			tracked_camera,
		})))
	}
}

/// Replaces camera calibration in the config with the values reported by OpenVR.
fn update_camera_config(system: &System, tracked_camera: &TrackedCamera) -> Result<()> {
	let hmd = tracked_device_index::HMD;
	
	if !tracked_camera.has_camera(hmd) {
		return Err(VRError::NoCamera.into());
	}
	
	let cameras = system.int32_tracked_device_property(hmd, TrackedDeviceProperty(sys::ETrackedDeviceProperty_Prop_NumCameras_Int32))?;
	let layout = system.int32_tracked_device_property(hmd, TrackedDeviceProperty(sys::ETrackedDeviceProperty_Prop_CameraFrameLayout_Int32))? as sys::EVRTrackedCameraFrameLayout;
	if cameras != 2 || layout & sys::EVRTrackedCameraFrameLayout_Stereo == 0 {
		return Err(VRError::UnsupportedCameraLayout(cameras, layout).into());
	}
	
	let frame_size = tracked_camera.get_camera_frame_size(hmd, FrameType::Distorted)?;
	let frame_size = vector!(frame_size.width as i32, frame_size.height as i32);
	let (eye_size, right_offset) = if layout & sys::EVRTrackedCameraFrameLayout_VerticalLayout != 0 {
		(vector!(frame_size.x, frame_size.y / 2), vector!(0, frame_size.y / 2))
	} else {
		(vector!(frame_size.x / 2, frame_size.y), vector!(frame_size.x / 2, 0))
	};
	
	let fn_table: &sys::VR_IVRSystem_FnTable = unsafe { &*load_fn_table(sys::IVRSystem_Version)? };
	let get_array = fn_table.GetArrayTrackedDeviceProperty.ok_or(VRError::MissingFunction("GetArrayTrackedDeviceProperty"))?;
	let get_array = |prop: sys::ETrackedDeviceProperty, tag: u32, buffer: &mut [u8]| -> Result<u32> {
		let mut error = sys::ETrackedPropertyError_TrackedProp_Success;
		let size = unsafe { get_array(hmd.0, prop, tag, buffer.as_mut_ptr() as *mut _, buffer.len() as u32, &mut error) };
		if error != sys::ETrackedPropertyError_TrackedProp_Success {
			return Err(VRError::Property(prop, error).into());
		}
		Ok(size)
	};
	
	// HmdMatrix34_t per camera
	let mut transforms = [[[0.0f32; 4]; 3]; 2];
	get_array(sys::ETrackedDeviceProperty_Prop_CameraToHeadTransforms_Matrix34_Array,
	          sys::k_unHmdMatrix34PropertyTag as u32,
	          bytemuck::cast_slice_mut(&mut transforms))?;
	
	// double[k_unMaxDistortionFunctionParameters] per camera (tagged as float array)
	let mut coeffs = [[0.0f64; sys::k_unMaxDistortionFunctionParameters as usize]; 2];
	get_array(sys::ETrackedDeviceProperty_Prop_CameraDistortionCoefficients_Float_Array,
	          sys::k_unFloatPropertyTag as u32,
	          bytemuck::cast_slice_mut(&mut coeffs))?;
	
	// Distorted frames have no intrinsics, but they are the same for undistorted ones.
	let intrinsics = [
		tracked_camera.get_camera_intrinsics(hmd, 0, FrameType::MaximumUndistorted)?,
		tracked_camera.get_camera_intrinsics(hmd, 1, FrameType::MaximumUndistorted)?,
	];
	
	config::rcu(|config| {
		config.camera.frame_buffer_size = frame_size;
		
		for (id, eye) in [&mut config.camera.left, &mut config.camera.right].into_iter().enumerate() {
			let m = &transforms[id];
			eye.offset = if id == 0 { vector!(0, 0) } else { right_offset };
			eye.size = eye_size;
			eye.focal_length = intrinsics[id].focal_length.into();
			eye.center = intrinsics[id].center.into();
			eye.coeffs = vector!(coeffs[id][0] as f32, coeffs[id][1] as f32, coeffs[id][2] as f32, coeffs[id][3] as f32);
			eye.position = Vec3::new(m[0][3], m[1][3], m[2][3]);
			eye.right = Vec3::new(m[0][0], m[1][0], m[2][0]);
			eye.back = Vec3::new(m[0][2], m[1][2], m[2][2]);
		}
	});
	
	Ok(())
}

impl Deref for VR {
	type Target = Mutex<VRInner>;
	
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}


#[derive(Debug, Error)]
pub enum VRError {
	#[error("OpenVR has already been initialized")] AlreadyInitialized,
	#[error("HMD has no camera")] NoCamera,
	#[error("Unsupported camera layout ({0} cameras, layout {1})")] UnsupportedCameraLayout(i32, sys::EVRTrackedCameraFrameLayout),
	#[error("OpenVR function {0} is missing")] MissingFunction(&'static str),
	#[error("Failed to read property {0} ({1})")] Property(sys::ETrackedDeviceProperty, sys::ETrackedPropertyError),
}
