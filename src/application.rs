use std::collections::HashMap;
use err_derive::Error;
use openvr::{System, Compositor, RenderModels, Context, InitError, tracked_device_index, TrackedDeviceClass, render_models, TrackedControllerRole};
use openvr::compositor::CompositorError;
use openvr::system::TrackedPropertyError;
use obj::ObjError;
use cgmath::{Matrix4, Vector3};

use crate::renderer::{Renderer, RendererCreationError, RenderError, camera};
use crate::renderer::model::{self, Model};
use crate::renderer::window::{Window, WindowCreationError};
use crate::openvr_vulkan::mat4;
use crate::debug::{get_debug_flag, set_debug_flag};

pub struct Application {
	context: Context,
	system: System,
	compositor: Compositor,
	render_models: RenderModels,
	renderer: Renderer,
	window: Window,
}


impl Application {
	pub fn new(device: Option<usize>, camera_api: CameraAPI) -> Result<Application, ApplicationCreationError> {
		
		let context = unsafe { openvr::init(openvr::ApplicationType::Scene) }?;
		
		let system = context.system()?;
		let compositor = context.compositor()?;
		let render_models = context.render_models()?;
		
		let renderer = match camera_api {
			CameraAPI::OpenCV => Renderer::new(&system, context.compositor()?, device, camera::OpenCV::new()?)?,
			CameraAPI::OpenVR => Renderer::new(&system, context.compositor()?, device, camera::OpenVR::new(&context)?)?,
			#[cfg(windows)] CameraAPI::Escapi => Renderer::new(&system, context.compositor()?, device, camera::Escapi::new()?)?,
			CameraAPI::Dummy => Renderer::new(&system, context.compositor()?, device, camera::Dummy::new())?,
		};
		
		let window = Window::new(&renderer)?;
		
		Ok(Application {
			context,
			system,
			compositor,
			render_models,
			renderer,
			window,
		})
	}
	
	pub fn run(mut self) -> Result<(), ApplicationRunError> {
		let mut scene: Vec<(Model, Matrix4<f32>)> = Vec::new();
		let mut devices: HashMap<u32, usize> = HashMap::new();
		
		let mut last_buttons = 0;
		
		scene.push((
			model::from_obj("models/cube/cube", &mut self.renderer)?,
			Matrix4::from_translation(Vector3::new(0.0, -1.5, -1.5)),
		));
		
		let rotating = scene.len();
		
		scene.push((
			model::from_pmx("models/YYB式初音ミクCrude Hair/YYB式初音ミクCrude Hair.pmx", &mut self.renderer)?,
			Matrix4::from_translation(Vector3::new(0.0, -1.0, -1.5)) * Matrix4::from_angle_y(cgmath::Deg(180.0)),
		));
		
		let mut rot = 0.0;
		
		
		while !self.window.quit_required {
			self.window.pull_events();
			
			let poses = self.compositor.wait_get_poses()?;
			
			for i in 0..poses.render.len() as u32 {
				if self.system.tracked_device_class(i) != TrackedDeviceClass::Invalid
				&& self.system.tracked_device_class(i) != TrackedDeviceClass::HMD {
					if devices.contains_key(&i) {
						scene[*devices.get(&i).unwrap()].1 = mat4(poses.render[i as usize].device_to_absolute_tracking());
					} else if let Some(model) = self.render_models.load_render_model(&self.system.string_tracked_device_property(i, 1003)?)? {
						if let Some(texture) = self.render_models.load_texture(model.diffuse_texture_id().unwrap())? {
							let model = model::from_openvr(model, texture, &mut self.renderer)?;
							
							devices.insert(i, scene.len());
							scene.push((model, mat4(poses.render[i as usize].device_to_absolute_tracking())));
							println!("Loaded {:?}", self.system.tracked_device_class(i));
						} else { break }
					} else { break }
				}
			}
			
			let buttons: u64 = [TrackedControllerRole::RightHand, TrackedControllerRole::LeftHand]
				.iter()
				.filter_map(|&role| self.system.tracked_device_index_for_controller_role(role))
				.filter_map(|index| self.system.controller_state(index))
				.map(|state| state.button_pressed)
				.fold(0, |a, b| a | b);
			
			let pressed = buttons & !last_buttons;
			last_buttons = buttons;
			
			for index in 0..64 {
				if pressed & (1 << index) != 0 {
					if index == 2 {
						let mode: u8 = get_debug_flag("mode").unwrap_or_default();
						set_debug_flag("mode", (mode + 1) % 3);
					}
				}
			}
			
			rot += 0.01;
			
			for (n, (_, orig)) in scene.iter_mut().enumerate() {
				if n == rotating {
					*orig = Matrix4::from_translation(Vector3::new(0.0, -1.0, -1.5)) * Matrix4::from_angle_y(cgmath::Rad(rot));
				}
			}
			
			let pose = poses.render[tracked_device_index::HMD as usize].device_to_absolute_tracking();
			
			self.renderer.render(pose, &mut scene, &mut self.window)?;
		}
		
		Ok(())
	}
}

impl Drop for Application {
	fn drop(&mut self) {
		// Context has to be shutdown before dropping graphical API
		unsafe { self.context.shutdown(); }
	}
}

pub enum CameraAPI {
	#[cfg(windows)] Escapi,
	OpenCV,
	OpenVR,
	Dummy,
}

#[derive(Debug, Error)]
pub enum ApplicationCreationError {
	#[error(display = "{}", _0)] OpenVRInitError(#[error(source)] InitError),
	#[error(display = "{}", _0)] RendererCreationError(#[error(source)] RendererCreationError),
	#[cfg(windows)] #[error(display = "{}", _0)] EscapiCameraError(#[error(source)] camera::EscapiCameraError),
	#[error(display = "{}", _0)] OpenCVCameraError(#[error(source)] camera::OpenCVCameraError),
	#[error(display = "{}", _0)] OpenVRCameraError(#[error(source)] camera::OpenVRCameraError),
	#[error(display = "{}", _0)] WindowCreationError(#[error(source)] WindowCreationError),
}

#[derive(Debug, Error)]
pub enum ApplicationRunError {
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] ModelError(#[error(source)] model::ModelError),
	#[error(display = "{}", _0)] ModelLoadError(#[error(source)] model::LoadError),
	#[error(display = "{}", _0)] CompositorError(#[error(source)] CompositorError),
	#[error(display = "{}", _0)] RenderError(#[error(source)] RenderError),
	#[error(display = "{}", _0)] TrackedPropertyError(#[error(source)] TrackedPropertyError),
	#[error(display = "{}", _0)] RenderModelError(#[error(source)] render_models::Error),
	#[error(display = "{}", _0)] ObjError(#[error(source)] ObjError),
}

