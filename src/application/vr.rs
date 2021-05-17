use openvr::{InitError, Context, System, Compositor, RenderModels};
use std::sync::Mutex;

pub type VR = Mutex<VRInner>;

pub struct VRInner {
	pub context: Context,
	pub system: System,
	pub compositor: Compositor,
	pub render_models: RenderModels,
}

impl VRInner {
	pub fn new() -> Result<VR, InitError> {
		let context = unsafe { openvr::init(openvr::ApplicationType::Scene) }?;
		
		let system = context.system()?;
		let compositor = context.compositor()?;
		let render_models = context.render_models()?;
		
		Ok(Mutex::new(VRInner{
			context,
			system,
			compositor,
			render_models,
		}))
	}
}

impl Drop for VRInner {
	fn drop(&mut self) {
		// Context has to be shutdown before dropping graphical API
		unsafe { self.context.shutdown(); }
	}
}
