use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;
use err_derive::Error;
use openvr::{MAX_TRACKED_DEVICE_COUNT, TrackedControllerRole, TrackedDeviceIndex};
use openvr::compositor::WaitPoses;
use rapier3d::prelude::ColliderBuilder;

pub mod vr;
pub mod entity;
pub mod physics;
pub mod input;

use crate::component::Component;
use crate::component::ComponentError;
use crate::component::model::ModelError;
use crate::component::parent::Parent;
use crate::component::pc_controlled::PCControlled;
use crate::component::pov::PoV;
use crate::component::toolgun::{ToolGun, ToolGunError};
use crate::component::vr::VrRoot;
// use crate::component::miku::Miku;
use crate::config::{self, CameraAPI};
use crate::math::{Color, Isometry3, PI, Rot3, Vec3};
use crate::renderer::{Renderer, RendererError, RendererRenderError};
use crate::renderer::assets_manager::obj::{ObjAsset, ObjLoadError};
use crate::renderer::camera::{self, OpenCVCameraError, OpenVRCameraError};
use crate::renderer::window::{Window, WindowCreationError};
use crate::utils::default_wait_poses;
use crate::debug;
pub use entity::{Entity, EntityRef};
pub use input::{Hand, Input, Key, MouseButton};
pub use physics::Physics;
pub use vr::{VR, VRError};
use crate::component::hand::HandComponent;
use openvr::tracked_device_index::HMD;

pub struct Application {
	pub vr: Option<Arc<VR>>,
	pub renderer: RefCell<Renderer>,
	pub physics: RefCell<Physics>,
	pub vr_poses: WaitPoses,
	pub camera_entity: EntityRef,
	pub input: Input,
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
		
		let application = Application {
			vr,
			renderer: RefCell::new(renderer),
			physics: RefCell::new(Physics::new()),
			vr_poses: default_wait_poses(),
			camera_entity: EntityRef::null(),
			input: Input::new(),
			window,
			entities: BTreeMap::new(),
			new_entities: RefCell::new(Vec::new()),
		};
		
		{
			let renderer = &mut application.renderer.borrow_mut();
			
			application.add_entity(
				Entity::builder("ඞ")
					.translation(point!(0.0, 20.0, 2.0))
					.build()
			);
			
			if application.vr.is_some() {
				application.add_entity(
					Entity::builder("VR Root")
						.component(VrRoot::new())
						.build()
				);
			} else {
				let pov = application.add_entity(
					Entity::builder("(You)")
						.translation(point!(0.0, 1.5, 1.5))
						.component(PoV::new())
						.component(PCControlled::new())
						.tag("Head", true)
						.build()
				);
				
				application.add_entity(
					Entity::builder("Hand")
						.component(renderer.load(ObjAsset::at("hand/hand_l.obj", "hand/hand_l.png"))?)
						.component(Parent::new(&pov, Isometry3::new(vector!(-0.2, -0.2, -0.4),
						                                            vector!(PI * 0.25, 0.0, 0.0))))
						.component(HandComponent::new(Hand::Left))
						.tag("Hand", Hand::Left)
						.build()
				);
				
				application.add_entity(
					Entity::builder("Hand")
						.component(renderer.load(ObjAsset::at("hand/hand_r.obj", "hand/hand_r.png"))?)
						.component(Parent::new(&pov, Isometry3::new(vector!(0.2, -0.2, -0.4),
						                                            vector!(PI * 0.25, 0.0, 0.0))))
						.component(HandComponent::new(Hand::Right))
						.tag("Hand", Hand::Right)
						.build()
				);
			}
			
			application.add_entity(
				Entity::builder("ToolGun")
					.translation(point!(0.0, 1.0, 1.0))
					.component(renderer.load(ObjAsset::at("toolgun/toolgun.obj", "toolgun/toolgun.png"))?)
					.component(ToolGun::new(Isometry3::from_parts(vector!(0.0, -0.03, 0.03).into(),
					                                              Rot3::from_euler_angles(PI * 0.25, PI, 0.0)),
					                        renderer)?)
					.collider_from_aabb()
					.build()
			);
			
			// application.add_entity(
			// 	Entity::builder("初音ミク")
			// 		.translation(point!(0.0, 0.0, 0.0))
			// 		.rotation(Rot3::from_euler_angles(0.0, std::f32::consts::PI, 0.0))
			// 		.component(Miku::new())
			// 		.build()
			// );
			
			application.add_entity(
				Entity::builder("Floor")
					.translation(point!(0.0, 0.0, 0.0))
					.component(renderer.load(ObjAsset::at("shapes/floor.obj", "shapes/floor.png"))?)
					.collider(ColliderBuilder::halfspace(Vec3::y_axis()).build())
					.tag("World", true)
					.hidden(config.camera.driver != CameraAPI::Dummy)
					.build()
			);
			
			// application.add_entity(
			// 	"Test",
			// 	crate::renderer::model::mmd::test::test_model(&mut renderer),
			// 	point!(2.0, 0.0, -1.5),
			// 	Rot3::from_euler_angles(0.0, 0.0, 0.0),
			// );
		}
		
		Ok(application)
	}
	
	pub fn run(mut self) -> Result<(), ApplicationRunError> {
		let mut instant = Instant::now();
		
		while !self.window.quit_required {
			let delta_time = instant.elapsed();
			instant = Instant::now();
			
			self.input.reset();
			self.window.pull_events(&mut self.input);
			if self.vr.is_some() {
				self.handle_vr_input()?;
			}
			self.set_debug_flags();
			
			let inputs = format!("{}", self.input);
			for (id, line) in inputs.split("\n").enumerate() {
				debug::draw_text(line, point!(-1.0, -1.0), debug::DebugOffset::bottom_right(16.0, 176.0 + id as f32 * 80.0), 64.0, Color::cyan());
			}
			
			self.setup_loop()?;
			
			{
				let physics = self.physics.get_mut();
				
				for entity in self.entities.values() {
					entity.before_physics(physics);
				}
				
				physics.step(delta_time);
				
				for entity in self.entities.values() {
					entity.after_physics(physics);
				}
			}
			
			for entity in self.entities.values() {
				entity.tick(delta_time, &self)?;
			}
			
			let pov = self.camera_entity.get(&self).map(|e| e.state().position)
			                            .unwrap_or(Isometry3::identity());
			
			let hmd_pose = self.vr_poses.render[HMD as usize].device_to_absolute_tracking().clone();
			
			self.renderer.get_mut().render(hmd_pose, pov, &mut self.entities, &mut self.window)?;
			
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
			
			for mut entity in self.new_entities.get_mut().drain(..) {
				let id = entity.id;
				
				entity.setup_physics(self.physics.get_mut());
				
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
		
		for (_, mut entity) in self.entities.drain_filter(|_, entity| entity.is_being_removed()) {
			entity.cleanup_physics(self.physics.get_mut());
		}
		
		Ok(())
	}
	
	fn handle_vr_input(&mut self) -> Result<(), ApplicationRunError> {
		let vr = self.vr.as_ref().expect("VR has not been initialized.").lock().unwrap();
		
		self.vr_poses = vr.compositor.wait_get_poses()?;
		
		if let Some(id) = vr.system.tracked_device_index_for_controller_role(TrackedControllerRole::LeftHand) {
			self.input.set_controller_id(Hand::Left, id);
		}
		
		if let Some(id) = vr.system.tracked_device_index_for_controller_role(TrackedControllerRole::RightHand) {
			self.input.set_controller_id(Hand::Right, id);
		}
		
		for id in 0..(MAX_TRACKED_DEVICE_COUNT as TrackedDeviceIndex) {
			if let Some(state) = vr.system.controller_state(id) {
				self.input.update_controller(id, state);
			}
		}
		
		Ok(())
	}
	
	fn set_debug_flags(&self) {
		debug::set_flag("DebugEntityDraw", self.input.keyboard.toggle(Key::N));
		debug::set_flag("DebugBoneDraw", self.input.keyboard.toggle(Key::B));
	}
}

#[derive(Debug, Error)]
pub enum ApplicationCreationError {
	#[error(display = "OpenvR unavailable. You can't use openvr background with --novr flag.")] OpenVRCameraInNoVR,
	#[error(display = "{}", _0)] RendererCreationError(#[error(source)] RendererError),
	#[error(display = "{}", _0)] VRError(#[error(source)] VRError),
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] ObjLoadError(#[error(source)] ObjLoadError),
	#[error(display = "{}", _0)] ToolGunError(#[error(source)] ToolGunError),
	#[error(display = "{}", _0)] OpenCVCameraError(#[error(source)] OpenCVCameraError),
	#[cfg(windows)] #[error(display = "{}", _0)] EscapiCameraError(#[error(source)] camera::EscapiCameraError),
	#[error(display = "{}", _0)] OpenVRCameraError(#[error(source)] OpenVRCameraError),
	#[error(display = "{}", _0)] WindowCreationError(#[error(source)] WindowCreationError),
}

#[derive(Debug, Error)]
pub enum ApplicationRunError {
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
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
