use std::sync::Mutex;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use err_derive::Error;
use openvr::{Context, System, Compositor, RenderModels};

mod tracked_camera;
mod camera_service;

pub use tracked_camera::{TrackedCamera, FrameType, TrackedCameraError};
pub use camera_service::{CameraService};

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
	pub fn new() -> Result<VR, VRError> {
		if VR_CREATED.swap(true, Ordering::SeqCst) {
			return Err(VRError::AlreadyInitialized);
		}
		
		let context = unsafe { openvr::init(openvr::ApplicationType::Scene) }?;
		
		let system = context.system()?;
		let compositor = context.compositor()?;
		let render_models = context.render_models()?;
		let tracked_camera = TrackedCamera::new(&context)?;
		
		Ok(VR(Mutex::new(VRInner{
			context,
			system,
			compositor,
			render_models,
			tracked_camera,
		})))
	}
}

impl Deref for VR {
	type Target = Mutex<VRInner>;
	
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}


#[derive(Debug, Error)]
pub enum VRError {
	#[error(display = "OpenVR has already been initialized")] AlreadyInitialized,
	#[error(display = "{}", _0)] OpenVRInitError(#[error(source)] openvr::InitError),
	#[error(display = "{}", _0)] TrackedCameraInitError(#[error(source)] tracked_camera::InitError),
}
