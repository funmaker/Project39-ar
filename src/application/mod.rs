use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;
use err_derive::Error;
use openvr::TrackedControllerRole;
use openvr::compositor::WaitPoses;
use rapier3d::dynamics::RigidBodyType;

pub use entity::{Entity, EntityRef};
pub use vr::{VR, VRError};
pub use physics::Physics;

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
use crate::component::toolgun::{ToolGun, ToolGunError};
use crate::component::parent::Parent;
use crate::component::physics::collider::ColliderComponent;

pub mod vr;
pub mod entity;
pub mod physics;

pub struct Application {
	pub vr: Option<Arc<VR>>,
	pub renderer: RefCell<Renderer>,
	pub physics: RefCell<Physics>,
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
		
		let application = Application {
			vr,
			renderer: RefCell::new(renderer),
			physics: RefCell::new(Physics::new()),
			vr_poses: default_wait_poses(),
			camera_entity: EntityRef::null(),
			window,
			entities: BTreeMap::new(),
			new_entities: RefCell::new(Vec::new()),
		};
		
		{
			let renderer = &mut application.renderer.borrow_mut();
			
			application.add_entity(
				Entity::builder("ඞ")
					.translation(Point3::new(0.0, 20.0, 2.0))
					.build()
			);
			
			if application.vr.is_some() {
				application.add_entity(
					Entity::builder("System")
						.translation(Point3::new(0.0, 0.0, -1.0))
						.component(VrSpawner::new())
						.build()
				);
			} else {
				let pov = application.add_entity(
					Entity::builder("(You)")
						.translation(Point3::new(0.0, 1.5, 1.5))
						.component(PoV::new())
						.component(PCControlled::new())
						.build()
				);
				
				application.add_entity(
					Entity::builder("Hand")
						.component(SimpleModel::<u16>::from_obj("hand/hand_l", renderer)?)
						.component(Parent::new(&pov, Isometry3::new(Vec3::new(-0.2, -0.2, -0.4),
						                                            Vec3::new(PI * 0.25, 0.0, 0.0))))
						.build()
				);
				
				application.add_entity(
					Entity::builder("Hand")
						.component(SimpleModel::<u16>::from_obj("hand/hand_r", renderer)?)
						.component(Parent::new(&pov, Isometry3::new(Vec3::new(0.2, -0.2, -0.4),
						                                            Vec3::new(PI * 0.25, 0.0, 0.0))))
						.build()
				);
			}
			
			application.add_entity(
				Entity::builder("ToolGun")
					.translation(Point3::new(0.0, 1.0, 1.0))
					.component(SimpleModel::<u16>::from_obj("toolgun/toolgun", renderer)?)
					.component(ToolGun::new(Isometry3::from_parts(Vec3::new(0.0, -0.03, 0.03).into(),
					                                              Rot3::from_euler_angles(PI * 0.25, PI, 0.0)),
					                        renderer)?)
					.build()
			);
			
			// application.add_entity(
			// 	Entity::builder("初音ミク")
			// 		.translation(Point3::new(0.0, 0.0, 0.0))
			// 		.rotation(Rot3::from_euler_angles(0.0, std::f32::consts::PI, 0.0))
			// 		.component(Miku::new())
			// 		.build()
			// );
			
			application.add_entity(
				Entity::builder("Floor")
					.translation(Point3::new(0.0, -0.5, 0.0))
					.component(ColliderComponent::cuboid(Vec3::new(10.0, 1.0, 10.0)))
					.build()
			);
			
			
			let box_model = SimpleModel::<u16>::from_obj("cube/cube", renderer)?;
			for id in 0..10 {
				application.add_entity(
					Entity::builder("Box")
						.rigid_body_type(RigidBodyType::Dynamic)
						.translation(Point3::new((id as f32).sin(), id as f32 * 1.5 + 0.5, (id as f32).cos()))
						.component(box_model.clone())
						.component(ColliderComponent::cuboid(Vec3::new(1.0, 1.0, 1.0)))
						.build()
				);
			}
			
			// application.add_entity(
			// 	"Test",
			// 	crate::renderer::model::mmd::test::test_model(&mut renderer),
			// 	Point3::new(2.0, 0.0, -1.5),
			// 	Rot3::from_euler_angles(0.0, 0.0, 0.0),
			// );
		}
		
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
	#[error(display = "{}", _0)] ToolGunError(#[error(source)] ToolGunError),
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
