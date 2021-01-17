use std::collections::HashMap;
use std::time::{Instant, Duration};
use std::sync::Arc;
use err_derive::Error;
use openvr::{tracked_device_index, TrackedDeviceClass, TrackedControllerRole};
use openvr_sys::{ETrackedDeviceProperty_Prop_RenderModelName_String, ETrackedDeviceProperty_Prop_TrackingSystemName_String};
use cgmath::num_traits::clamp;
use cgmath::{Vector3, Quaternion, One, Zero, Decomposed, Euler, Rad, Angle, Rotation3, Matrix4};

mod vr;
mod entity;

use crate::renderer::{Renderer, RendererError, RendererRenderError};
use crate::renderer::window::{Window, WindowCreationError};
use crate::renderer::camera::{self, OpenCVCameraError, OpenVRCameraError};
use crate::renderer::model::{self, ModelLoadError, ModelError};
use crate::debug::{get_flag, set_flag, get_flag_or_default};
use crate::utils::mat4;
pub use vr::VR;
pub use entity::Entity;

pub struct Application {
	vr: Option<Arc<VR>>,
	renderer: Renderer,
	window: Window,
	scene: Vec<Entity>,
	vr_devices: HashMap<u32, usize>,
}

type FakePose = (Vector3<f32>, Euler<Rad<f32>>);

impl Application {
	pub fn new(device: Option<usize>, camera_api: CameraAPI, vr: bool) -> Result<Application, ApplicationCreationError> {
		let vr = vr.then(|| VR::new())
		           .transpose()?
		           .map(Arc::new);
		
		if vr.is_none() && camera_api == CameraAPI::OpenVR {
			return Err(ApplicationCreationError::OpenVRCameraInNoVR);
		}
		
		let mut renderer = match camera_api {
			CameraAPI::OpenCV => Renderer::new(vr.clone(), device, camera::OpenCV::new()?)?,
			CameraAPI::OpenVR => Renderer::new(vr.clone(), device, camera::OpenVR::new(&vr.as_ref().unwrap())?)?,
			#[cfg(windows)] CameraAPI::Escapi => Renderer::new(vr.clone(), device, camera::Escapi::new()?)?,
			CameraAPI::Dummy => Renderer::new(vr.clone(), device, camera::Dummy::new())?,
		};
		
		let window = Window::new(&renderer)?;
		
		let mut scene: Vec<Entity> = Vec::new();
		
		scene.push(Entity::new(
			"Cube",
			model::from_obj::<u16>("models/cube/cube", &mut renderer)?,
			Vector3::new(0.0, -1.5, -1.5),
			Quaternion::one(),
		));
		
		scene.push(Entity::new(
			"初音ミク",
			model::from_pmx("models/YYB式初音ミクCrude Hair/YYB式初音ミクCrude Hair.pmx", &mut renderer)?,
			Vector3::new(0.0, -1.0, -1.5),
			Quaternion::from_angle_y(Rad::turn_div_2()),
		));
		
		Ok(Application {
			vr,
			renderer,
			window,
			scene,
			vr_devices: HashMap::new(),
		})
	}
	
	pub fn run(mut self) -> Result<(), ApplicationRunError> {
		let mut instant = Instant::now();
		
		let mut vr_buttons = 0;
		let mut fake_pose = (Vector3::zero(), Euler{ x: Rad(0.0), y: Rad(0.0), z: Rad(0.0) });
		
		while !self.window.quit_required {
			self.window.pull_events();
			
			let delta_time = instant.elapsed();
			instant = Instant::now();
			
			let pose = match self.vr {
				Some(_) => self.handle_vr_poses(&mut vr_buttons)?,
				None => self.handle_fake_pose(&mut fake_pose, delta_time),
			};
			
			for entity in self.scene.iter_mut() {
				entity.tick(delta_time);
			}
			
			self.renderer.render(pose, &mut self.scene, &mut self.window)?;
		}
		
		Ok(())
	}
	
	fn handle_vr_poses(&mut self, last_buttons: &mut u64) -> Result<Matrix4<f32>, ApplicationRunError> {
		let vr = self.vr.as_ref().expect("VR has not been initialized.");
		
		let poses = vr.compositor.wait_get_poses()?;
		
		for i in 0..poses.render.len() as u32 {
			if vr.system.tracked_device_class(i) != TrackedDeviceClass::Invalid && vr.system.tracked_device_class(i) != TrackedDeviceClass::HMD {
				if let Some(&id) = self.vr_devices.get(&i) {
					self.scene[id].move_to_pose(poses.render[i as usize]);
				} else if let Some(model) = vr.render_models.load_render_model(&vr.system.string_tracked_device_property(i, ETrackedDeviceProperty_Prop_RenderModelName_String)?)? {
					if let Some(texture) = vr.render_models.load_texture(model.diffuse_texture_id().unwrap())? {
						let mut entity = Entity::new(
							vr.system.string_tracked_device_property(i, ETrackedDeviceProperty_Prop_TrackingSystemName_String)?.to_string_lossy(),
							model::from_openvr(model, texture, &mut self.renderer)?,
							Vector3::zero(),
							Quaternion::one(),
						);
						
						entity.move_to_pose(poses.render[i as usize]);
						self.vr_devices.insert(i, self.scene.len());
						self.scene.push(entity);
						println!("Loaded {:?}", vr.system.tracked_device_class(i));
					} else { break }
				} else { break }
			}
		}
		
		let buttons: u64 = [TrackedControllerRole::RightHand, TrackedControllerRole::LeftHand]
			.iter()
			.filter_map(|&role| vr.system.tracked_device_index_for_controller_role(role))
			.filter_map(|index| vr.system.controller_state(index))
			.map(|state| state.button_pressed)
			.fold(0, |a, b| a | b);
		
		let pressed = buttons & !*last_buttons;
		*last_buttons = buttons;
		
		for index in 0..64 {
			if pressed & (1 << index) != 0 {
				if index == 2 {
					let mode: u8 = get_flag_or_default("mode");
					set_flag("mode", (mode + 1) % 3);
				}
			}
		}
		
		Ok(mat4(poses.render[tracked_device_index::HMD as usize].device_to_absolute_tracking()))
	}
	
	fn handle_fake_pose(&self, fake_pose: &mut FakePose, delta_time: Duration) -> Matrix4<f32> {
		let (disp, rot) = fake_pose;
		
		fn get_key(key: &str) -> f32 {
			get_flag_or_default::<bool>(key) as i32 as f32
		}
		
		let x = get_key("KeyD") - get_key("KeyA");
		let y = get_key("KeySpace") - get_key("KeyCtrl");
		let z = get_key("KeyS") - get_key("KeyW");
		let dist = (0.5 + get_key("KeyLShift") * 1.0) * delta_time.as_secs_f32();
		let mouse_move = get_flag("mouse_move").unwrap_or((0.0_f32, 0.0_f32));
		set_flag("mouse_move", (0.0_f32, 0.0_f32));
		
		rot.y = rot.y + Rad(-mouse_move.0 * 0.01);
		rot.x = clamp(rot.x + Rad(-mouse_move.1 * 0.01), -Rad::turn_div_4(), Rad::turn_div_4());
		
		let quat = Quaternion::from_angle_y(rot.y) * Quaternion::from_angle_x(rot.x);
		
		disp.y += y * dist;
		*disp += quat * Vector3::new(x, 0.0, z) * dist;
		
		// Y * X rotation
		Decomposed {
			scale: 1.0,
			rot: quat,
			disp: disp.clone(),
		}.into()
	}
}

#[derive(Debug, Eq, PartialEq)]
pub enum CameraAPI {
	#[cfg(windows)] Escapi,
	OpenCV,
	OpenVR,
	Dummy,
}

#[derive(Debug, Error)]
pub enum ApplicationCreationError {
	#[error(display = "OpenvR unavailable. You can't use openvr camera with --novr flag.")] OpenVRCameraInNoVR,
	#[error(display = "{}", _0)] RendererCreationError(#[error(source)] RendererError),
	#[error(display = "{}", _0)] ModelLoadError(#[error(source)] ModelLoadError),
	#[error(display = "{}", _0)] OpenCVCameraError(#[error(source)] OpenCVCameraError),
	#[cfg(windows)] #[error(display = "{}", _0)] EscapiCameraError(#[error(source)] camera::EscapiCameraError),
	#[error(display = "{}", _0)] OpenVRCameraError(#[error(source)] OpenVRCameraError),
	#[error(display = "{}", _0)] WindowCreationError(#[error(source)] WindowCreationError),
	#[error(display = "{}", _0)] OpenVRInitError(#[error(source)] openvr::InitError),
}

#[derive(Debug, Error)]
pub enum ApplicationRunError {
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] ModelLoadError(#[error(source)] ModelLoadError),
	#[error(display = "{}", _0)] RenderError(#[error(source)] RendererRenderError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] CompositorError(#[error(source)] openvr::compositor::CompositorError),
	#[error(display = "{}", _0)] TrackedPropertyError(#[error(source)] openvr::system::TrackedPropertyError),
	#[error(display = "{}", _0)] RenderModelError(#[error(source)] openvr::render_models::Error),
	#[error(display = "{}", _0)] ObjError(#[error(source)] obj::ObjError),
}

