use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;
use err_derive::Error;
use openvr::TrackedControllerRole;
use openvr::compositor::WaitPoses;

pub use entity::{Entity, EntityRef};
pub use vr::{VR, VRError};

use crate::component::Component;
use crate::utils::default_wait_poses;
use crate::config::{self, CameraAPI};
use crate::debug;
use crate::math::{Isometry3, Point3, Rot3, Vec3, PI};
use crate::renderer::{Renderer, RendererError, RendererRenderError};
use crate::renderer::camera::{self, OpenCVCameraError, OpenVRCameraError};
use crate::renderer::window::{Window, WindowCreationError};
use crate::component::model::{mmd::MMDModelLoadError, ModelError, simple::SimpleModelLoadError, SimpleModel};
use crate::component::vr::VrSpawner;
use crate::component::miku::Miku;
use crate::component::ComponentError;
use crate::component::pov::PoV;
use crate::component::pc_controlled::PCControlled;
use crate::component::toolgun::ToolGun;
use crate::component::parent::Parent;

pub mod vr;
pub mod entity;

pub struct Application {
	pub vr: Option<Arc<VR>>,
	pub renderer: RefCell<Renderer>,
	pub vr_poses: WaitPoses,
	pub camera_entity: EntityRef,
	window: Window,
	entities: BTreeMap<u64, Entity>,
	new_entities: RefCell<Vec<Entity>>,
}

impl Application {
	pub fn new() -> Result<Application, ApplicationCreationError> {
		let config = config::get();
		let vr = (!config.novr.enabled).then(|| VR::new())
		                               .transpose()?
		                               .map(Arc::new);
		
		if vr.is_none() && config.camera.driver == CameraAPI::OpenVR {
			return Err(ApplicationCreationError::OpenVRCameraInNoVR);
		}
		
		let renderer = match config.camera.driver {
			CameraAPI::OpenCV => Renderer::new(vr.clone(), camera::OpenCV::new()?)?,
			CameraAPI::OpenVR => Renderer::new(vr.clone(), camera::OpenVR::new(vr.clone().unwrap())?)?,
			#[cfg(windows)] CameraAPI::Escapi => Renderer::new(vr.clone(), camera::Escapi::new()?)?,
			CameraAPI::Dummy => Renderer::new(vr.clone(), camera::Dummy::new())?,
		};
		
		let window = Window::new(&renderer)?;
		
		let vr_poses = default_wait_poses();
		
		let mut application = Application {
			vr,
			renderer: RefCell::new(renderer),
			vr_poses,
			camera_entity: EntityRef::null(),
			window,
			entities: BTreeMap::new(),
			new_entities: RefCell::new(Vec::new()),
		};
		
		application.add_entity(Entity::new(
			"ඞ",
			Point3::new(0.0, 20.0, 2.0),
			Rot3::identity(),
			None,
		));
		
		if application.vr.is_some() {
			application.add_entity(Entity::new(
				"System",
				Point3::new(0.0, 0.0, -1.0),
				Rot3::identity(),
				Some(VrSpawner::new().boxed()),
			));
		} else {
			let pov = application.add_entity(Entity::new(
				"(You)",
				Point3::new(0.0, 1.5, 1.5),
				Rot3::identity(),
				[PoV::new().boxed(), PCControlled::new().boxed()],
			));
			
			let model = SimpleModel::<u16>::from_obj("hand/hand_l", application.renderer.get_mut())?.boxed();
			application.add_entity(Entity::new(
				"Hand",
				Point3::new(0.0, 0.0, 0.0),
				Rot3::identity(),
				[
					model,
					Parent::new(&pov, Isometry3::new(Vec3::new(-0.2, -0.2, -0.4),
					                                 Vec3::new(PI * 0.25, 0.0, 0.0))).boxed(),
				],
			));
			
			let model = SimpleModel::<u16>::from_obj("hand/hand_r", application.renderer.get_mut())?.boxed();
			application.add_entity(Entity::new(
				"Hand",
				Point3::new(0.0, 0.0, 0.0),
				Rot3::identity(),
				[
					model,
					Parent::new(&pov, Isometry3::new(Vec3::new(0.2, -0.2, -0.4),
					                                 Vec3::new(PI * 0.25, 0.0, 0.0))).boxed(),
				],
			));
		}
		
		let model = SimpleModel::<u16>::from_obj("toolgun/toolgun", application.renderer.get_mut())?.boxed();
		application.add_entity(Entity::new(
			"ToolGun",
			Point3::new(0.0, 1.0, 1.0),
			Rot3::identity(),
			[model, ToolGun::new(Isometry3::from_parts(Vec3::new(0.0, -0.03, 0.03).into(), Rot3::from_euler_angles(PI * 0.25, PI, 0.0))).boxed()],
		));
		
		application.add_entity(Entity::new(
			"初音ミク",
			Point3::new(0.0, 0.0, 0.0),
			Rot3::from_euler_angles(0.0, std::f32::consts::PI, 0.0),
			Some(Miku::new().boxed()),
		));
		
		// application.add_entity(
		// 	"Test",
		// 	crate::renderer::model::mmd::test::test_model(&mut renderer),
		// 	Point3::new(2.0, 0.0, -1.5),
		// 	Rot3::from_euler_angles(0.0, 0.0, 0.0),
		// );
		
		Ok(application)
	}
	
	pub fn run(mut self) -> Result<(), ApplicationRunError> {
		let mut instant = Instant::now();
		
		let mut vr_buttons = 0;
		
		while !self.window.quit_required {
			self.window.pull_events();
			
			let delta_time = instant.elapsed();
			instant = Instant::now();
			
			if self.vr.is_some() {
				self.handle_vr_poses(&mut vr_buttons)?;
			}
			
			self.setup_loop()?;
			
			for entity in self.entities.values() {
				entity.do_physics(delta_time);
			}
			
			for entity in self.entities.values() {
				entity.tick(delta_time, &self)?;
			}
			
			let pov = self.camera_entity.get(&self).map(|e| e.state().position)
			                            .unwrap_or(Isometry3::identity());
			
			self.renderer.get_mut().render(pov, &mut self.entities, &mut self.window)?;
			
			self.cleanup_loop()?;
		}
		
		Ok(())
	}
	
	#[allow(dead_code)]
	pub fn add_entity(&self, entity: Entity) -> EntityRef {
		let entity_ref = entity.as_ref();
		
		self.new_entities.borrow_mut().push(entity);
		
		entity_ref
	}
	
	#[allow(dead_code)]
	pub fn entity(&self, id: u64) -> Option<&Entity> {
		self.entities.get(&id)
	}
	
	#[allow(dead_code)]
	pub fn find_all_entities(&self, predicate: impl Fn(&Entity) -> bool) -> impl Iterator<Item = &Entity> {
		self.entities
		    .values()
		    .filter(move |entity| entity.try_state().is_some() && predicate(entity))
	}
	
	#[allow(dead_code)]
	pub fn find_entity(&self, predicate: impl Fn(&Entity) -> bool) -> Option<&Entity> {
		self.find_all_entities(predicate).next()
	}
	
	fn setup_loop(&mut self) -> Result<(), ApplicationRunError> {
		let mut clean = false;
		while !clean {
			clean = true;
			
			for entity in self.new_entities.get_mut().drain(..) {
				let id = entity.id;
				
				let old = self.entities.insert(id, entity);
				assert!(old.is_none(), "Entity id {} already taken!", id);
			}
			
			let unsafe_ref = unsafe { &*(self as *const Self) }; // TODO: This is unsafe. Maybe split?
			for entity in self.entities.values_mut() {
				if entity.setup_components(unsafe_ref)? {
					clean = false;
				}
			}
		}
		
		Ok(())
	}
	
	fn cleanup_loop(&mut self) -> Result<(), ComponentError> {
		let mut clean = false;
		while !clean {
			clean = true;
			
			let unsafe_ref = unsafe { &*(self as *const Self) }; // TODO: This is unsafe. Maybe split?
			
			for entity in self.entities.values_mut() {
				if entity.cleanup_components(unsafe_ref)? {
					clean = false;
				}
			}
		}
		
		self.entities.drain_filter(|_, entity| entity.is_being_removed());
		
		Ok(())
	}
	
	fn handle_vr_poses(&mut self, last_buttons: &mut u64) -> Result<(), ApplicationRunError> {
		let vr = self.vr.as_ref().expect("VR has not been initialized.").lock().unwrap();
		
		self.vr_poses = vr.compositor.wait_get_poses()?;
		
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
		
		Ok(())
	}
}

#[derive(Debug, Error)]
pub enum ApplicationCreationError {
	#[error(display = "OpenvR unavailable. You can't use openvr background with --novr flag.")] OpenVRCameraInNoVR,
	#[error(display = "{}", _0)] RendererCreationError(#[error(source)] RendererError),
	#[error(display = "{}", _0)] VRError(#[error(source)] VRError),
	#[error(display = "{}", _0)] MMDModelLoadError(#[error(source)] MMDModelLoadError),
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] SimpleModelLoadError(#[error(source)] SimpleModelLoadError),
	#[error(display = "{}", _0)] OpenCVCameraError(#[error(source)] OpenCVCameraError),
	#[cfg(windows)] #[error(display = "{}", _0)] EscapiCameraError(#[error(source)] camera::EscapiCameraError),
	#[error(display = "{}", _0)] OpenVRCameraError(#[error(source)] OpenVRCameraError),
	#[error(display = "{}", _0)] WindowCreationError(#[error(source)] WindowCreationError),
}

#[derive(Debug, Error)]
pub enum ApplicationRunError {
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] SimpleModelLoadError(#[error(source)] SimpleModelLoadError),
	#[error(display = "{}", _0)] RendererRenderError(#[error(source)] RendererRenderError),
	#[error(display = "{}", _0)] ComponentError(ComponentError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] CompositorError(#[error(source)] openvr::compositor::CompositorError),
	#[error(display = "{}", _0)] TrackedPropertyError(#[error(source)] openvr::system::TrackedPropertyError),
	#[error(display = "{}", _0)] RenderModelError(#[error(source)] openvr::render_models::Error),
	#[error(display = "{}", _0)] ObjError(#[error(source)] obj::ObjError),
}

impl From<ComponentError> for ApplicationRunError {
	fn from(err: ComponentError) -> Self {
		ApplicationRunError::ComponentError(err)
	}
}
