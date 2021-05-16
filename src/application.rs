use std::collections::HashMap;
use std::time::{Instant, Duration};
use std::sync::Arc;
use err_derive::Error;
use openvr::{tracked_device_index, TrackedDeviceClass, TrackedControllerRole};
use openvr_sys::{ETrackedDeviceProperty_Prop_RenderModelName_String, ETrackedDeviceProperty_Prop_TrackingSystemName_String};
use simba::scalar::SupersetOf;

mod vr;
pub mod entity;

use crate::renderer::{Renderer, RendererError, RendererRenderError};
use crate::renderer::window::{Window, WindowCreationError};
use crate::renderer::camera::{self, OpenCVCameraError, OpenVRCameraError};
use crate::renderer::model::{ModelError, SimpleModel, MMDModel, mmd::MMDModelLoadError, simple::SimpleModelLoadError, Model};
use crate::math::{Vec3, Rot3, Isometry3, AMat4, Point3, ToTransform, Translation3};
use crate::debug;
pub use vr::VR;
pub use entity::Entity;

pub struct Application {
	vr: Option<Arc<VR>>,
	renderer: Renderer,
	window: Window,
	scene: Vec<Entity>,
	vr_devices: HashMap<u32, usize>,
}

type FakePose = (Translation3, (f32, f32));

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
			SimpleModel::<u16>::from_obj("models/cube/cube", &mut renderer)?,
			Point3::new(0.0, -1.5, -1.5),
			Rot3::identity(),
		));
		
		scene.push(Entity::new(
			"初音ミク",
			MMDModel::<u16>::from_pmx("models/YYB式初音ミクCrude Hair/YYB式初音ミクCrude Hair.pmx", &mut renderer)?,
			Point3::new(0.0, -1.0, -1.5),
			Rot3::from_euler_angles(0.0, std::f32::consts::PI, 0.0),
		));
		
		scene.push(Entity::new(
			"Test",
			crate::renderer::model::mmd::test::test_model(&mut renderer),
			Point3::new(2.0, -1.0, -3.0),
			Rot3::from_euler_angles(0.0, 0.0, 0.0),
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
		let mut fake_pose: FakePose = (Translation3::identity(), (0.0, 0.0));
		
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
	
	fn handle_vr_poses(&mut self, last_buttons: &mut u64) -> Result<Isometry3, ApplicationRunError> {
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
							SimpleModel::<u16>::from_openvr(model, texture, &mut self.renderer)?,
							Point3::origin(),
							Rot3::identity(),
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
					let mode: u8 = debug::get_flag_or_default("mode");
					debug::set_flag("mode", (mode + 1) % 3);
				}
			}
		}
		
		let orientation: AMat4 = poses.render[tracked_device_index::HMD as usize].device_to_absolute_tracking().to_transform();
		Ok(orientation.to_subset().unwrap())
	}
	
	fn handle_fake_pose(&self, fake_pose: &mut FakePose, delta_time: Duration) -> Isometry3 {
		let (position, (pitch, yaw)) = fake_pose;
		
		fn get_key(key: &str) -> f32 {
			debug::get_flag_or_default::<bool>(key) as i32 as f32
		}
		
		let x = get_key("KeyD") - get_key("KeyA");
		let y = get_key("KeySpace") - get_key("KeyCtrl");
		let z = get_key("KeyS") - get_key("KeyW");
		let dist = (0.5 + get_key("KeyLShift") * 1.0) * delta_time.as_secs_f32();
		let mouse_move = debug::get_flag("mouse_move").unwrap_or((0.0_f32, 0.0_f32));
		debug::set_flag("mouse_move", (0.0_f32, 0.0_f32));
		
		*yaw = *yaw + -mouse_move.0 * 0.01;
		*pitch = (*pitch + -mouse_move.1 * 0.01).clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
		
		let rot = Rot3::from_euler_angles(*pitch, *yaw, 0.0);
		let disp = rot * Vec3::new(x, 0.0, z) * dist + Vec3::y() * y * dist;
		
		position.vector += disp;
		
		Isometry3::from_parts(position.clone(), rot)
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
	#[error(display = "{}", _0)] MMDModelLoadError(#[error(source)] MMDModelLoadError),
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] SimpleModelLoadError(#[error(source)] SimpleModelLoadError),
	#[error(display = "{}", _0)] OpenCVCameraError(#[error(source)] OpenCVCameraError),
	#[cfg(windows)] #[error(display = "{}", _0)] EscapiCameraError(#[error(source)] camera::EscapiCameraError),
	#[error(display = "{}", _0)] OpenVRCameraError(#[error(source)] OpenVRCameraError),
	#[error(display = "{}", _0)] WindowCreationError(#[error(source)] WindowCreationError),
	#[error(display = "{}", _0)] OpenVRInitError(#[error(source)] openvr::InitError),
}

#[derive(Debug, Error)]
pub enum ApplicationRunError {
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] SimpleModelLoadError(#[error(source)] SimpleModelLoadError),
	#[error(display = "{}", _0)] RendererRenderError(#[error(source)] RendererRenderError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] CompositorError(#[error(source)] openvr::compositor::CompositorError),
	#[error(display = "{}", _0)] TrackedPropertyError(#[error(source)] openvr::system::TrackedPropertyError),
	#[error(display = "{}", _0)] RenderModelError(#[error(source)] openvr::render_models::Error),
	#[error(display = "{}", _0)] ObjError(#[error(source)] obj::ObjError),
}

