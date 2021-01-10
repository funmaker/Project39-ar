use std::collections::HashMap;
use openvr::{InitError, Context, System, Compositor, RenderModels};

pub struct VR {
	pub context: Context,
	pub system: System,
	pub compositor: Compositor,
	pub render_models: RenderModels,
	pub devices: HashMap<u32, usize>,
}

impl VR {
	pub fn new() -> Result<VR, InitError> {
		let context = unsafe { openvr::init(openvr::ApplicationType::Scene) }?;
		
		let system = context.system()?;
		let compositor = context.compositor()?;
		let render_models = context.render_models()?;
		
		Ok(VR{
			context,
			system,
			compositor,
			render_models,
			devices: HashMap::new(),
		})
	}
}

impl Drop for VR {
	fn drop(&mut self) {
		// Context has to be shutdown before dropping graphical API
		unsafe { self.context.shutdown(); }
	}
}
