use std::ops::Deref;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Result;
use thiserror::Error;
use openvr::{Context, System, Compositor, RenderModels};

mod camera_service;
mod tracked_camera;

pub use camera_service::CameraService;
pub use tracked_camera::{TrackedCamera, FrameType};


/// Guards against initializing OpenVR more than once at a time.
static VR_CREATED: AtomicBool = AtomicBool::new(false);

/// OpenVR context and interfaces. Access it by locking `VR`.
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

/// Thread-safe handle to the OpenVR runtime. Only one instance can exist at a time.
pub struct VR(Mutex<VRInner>);

impl VR {
	/// Initializes OpenVR as a scene application and loads its interfaces.
	pub fn new() -> Result<VR> {
		if VR_CREATED.swap(true, Ordering::SeqCst) {
			return Err(VRError::AlreadyInitialized.into());
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
	#[error("OpenVR has already been initialized")] AlreadyInitialized,
}
