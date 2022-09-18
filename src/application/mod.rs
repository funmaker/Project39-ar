use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use err_derive::Error;
use openvr::{MAX_TRACKED_DEVICE_COUNT, TrackedControllerRole, TrackedDeviceIndex};
use openvr::compositor::WaitPoses;
use openvr::tracked_device_index::HMD;
use rapier3d::dynamics::{GenericJoint, JointAxesMask, JointAxis, RigidBodyType};
use rapier3d::prelude::ColliderBuilder;

pub mod vr;
pub mod entity;
pub mod physics;
pub mod input;
mod eyes;
mod window;

use crate::component::Component;
use crate::component::ComponentError;
use crate::component::parent::Parent;
use crate::component::pc_controlled::PCControlled;
use crate::component::pov::PoV;
use crate::component::vr::VrRoot;
use crate::component::miku::Miku;
use crate::component::hand::HandComponent;
use crate::component::physics::joint::JointComponent;
use crate::config::{self, CameraAPI};
use crate::math::{Color, Isometry3, PI, Rot3, Vec3};
use crate::renderer::{Renderer, RendererBeginFrameError, RendererEndFrameError, RendererError, RendererRenderError, RenderTarget};
use crate::component::model::simple::asset::{ObjAsset, ObjLoadError};
use crate::utils::default_wait_poses;
use crate::debug;
pub use entity::{Entity, EntityRef};
pub use input::{Hand, Input, Key, MouseButton};
pub use physics::Physics;
pub use vr::{VR, VRError};
use eyes::{camera, Eyes, EyesLoadBackgroundError, EyesCreationError, EyesRenderTargetError};
use window::{Window, WindowCreationError, WindowMirrorFromError, WindowRenderTargetError, WindowSwapchainRegenError};


pub struct Application {
	pub vr: Option<Arc<VR>>,
	pub renderer: RefCell<Renderer>,
	pub physics: RefCell<Physics>,
	pub vr_poses: WaitPoses,
	pub pov: EntityRef,
	pub detached_pov: EntityRef,
	pub input: Input,
	eyes: Option<Eyes>,
	window: Option<Window>,
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
		
		let mut renderer = Renderer::new(vr.clone())?;
		
		let camera: Box<dyn camera::Camera> = match config.camera.driver {
			#[cfg(feature = "opencv-camera")]
			CameraAPI::OpenCV => Box::new(camera::OpenCV::new()?),
			CameraAPI::OpenVR => Box::new(camera::OpenVR::new(vr.clone().unwrap())?),
			#[cfg(windows)]
			CameraAPI::Escapi => Box::new(camera::Escapi::new()?),
			CameraAPI::Dummy => Box::new(camera::Dummy::new()),
		};
		
		let eyes = if let Some(ref vr) = vr {
			Eyes::new_vr(vr.clone(), Some(camera), &mut renderer)?
		} else {
			Eyes::new_novr(&config::get().novr, Some(camera), &mut renderer)?
		};
		
		let window = Window::new(Some(eyes.framebuffer_size()), &renderer)?;
		
		let application = Application {
			vr,
			renderer: RefCell::new(renderer),
			physics: RefCell::new(Physics::new()),
			vr_poses: default_wait_poses(),
			pov: EntityRef::null(),
			detached_pov: EntityRef::null(),
			input: Input::new(),
			eyes: Some(eyes),
			window: Some(window),
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
						.component(PoV::new(true))
						.component(PCControlled::new())
						.tag("Head", true)
						.build()
				);
				
				application.add_entity(
					Entity::builder("Hand")
						.component(renderer.load(ObjAsset::at("hand/hand_l.obj", "hand/hand_l.png"))?)
						.component(Parent::new(&pov, Isometry3::new(vector!(-0.2, -0.2, -0.4).into(),
						                                            vector!(PI * 0.25, 0.0, 0.0))))
						.component(HandComponent::new(Hand::Left))
						.collider_from_aabb(1000.0)
						.tag("Hand", Hand::Left)
						.build()
				);
				
				application.add_entity(
					Entity::builder("Hand")
						.component(renderer.load(ObjAsset::at("hand/hand_r.obj", "hand/hand_r.png"))?)
						.component(Parent::new(&pov, Isometry3::new(vector!(0.2, -0.2, -0.4).into(),
						                                            vector!(PI * 0.25, 0.0, 0.0))))
						.component(HandComponent::new(Hand::Right))
						.collider_from_aabb(1000.0)
						.tag("Hand", Hand::Right)
						.build()
				);
			}
			
			// application.add_entity(
			// 	Entity::builder("ToolGun")
			// 		.translation(point!(0.0, 1.0, 1.0))
			// 		.component(renderer.load(ObjAsset::at("toolgun/toolgun.obj", "toolgun/toolgun.png"))?)
			// 		.component(ToolGun::new(Isometry3::from_parts(vector!(0.0, -0.03, 0.03).into(),
			// 		                                              Rot3::from_euler_angles(PI * 0.25, PI, 0.0)),
			// 		                        renderer)?)
			// 		.collider_from_aabb()
			// 		.build()
			// );
			
			application.add_entity(
				Entity::builder("初音ミク")
					.translation(point!(-0.5, 0.0, 0.0))
					.rotation(Rot3::from_euler_angles(0.0, PI * 0.0, 0.0))
					.component(Miku::new())
					.build()
			);
			
			// application.add_entity(
			// 	Entity::builder("Test")
			// 		.translation(point!(-3.0, 3.0, -3.0))
			// 		.rotation(Rot3::from_euler_angles(0.0, 0.0, 0.0))
			// 		.component(MMDModel::new(renderer.load(PmxAsset::at("test2/test2.pmx"))?, renderer)?)
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
			
			let box1 = application.add_entity(
				Entity::builder("Box")
					.position(Isometry3::new(vector!(0.0, 1.875, 1.0), vector!(0.0, 0.0, 0.0)))
					.component(renderer.load(ObjAsset::at("shapes/box/box_1x1x1.obj", "shapes/textures/box.png"))?)
					.collider_from_aabb(100.0)
					.rigid_body_type(RigidBodyType::Dynamic)
					.build()
			);
			
			let mut joint = GenericJoint::new(JointAxesMask::X | JointAxesMask::Y | JointAxesMask::Z);
			
			joint.set_local_frame1(Isometry3::new(vector!(0.0, 0.125, 0.0), vector!(0.0, 0.0, 0.0)))
			     .set_local_frame2(Isometry3::new(vector!(0.0, -0.625, 0.0), vector!(0.0, 0.0, 0.0)))
			     .set_limits(JointAxis::AngZ, [-30.0 / 180.0 * PI, 30.0 / 180.0 * PI])
			     .set_limits(JointAxis::AngY, [-30.0 / 180.0 * PI, 30.0 / 180.0 * PI])
			     .set_limits(JointAxis::AngZ, [-30.0 / 180.0 * PI, 30.0 / 180.0 * PI]);
			
			application.add_entity(
				Entity::builder("Box")
					.position(Isometry3::new(vector!(0.0, 1.0, 1.0), vector!(0.0, 0.0, 0.0)))
					.component(renderer.load(ObjAsset::at("shapes/box/box_1x1x1.obj", "shapes/textures/box.png"))?)
					.component(JointComponent::new(joint, box1))
					.collider_from_aabb(1000.0)
					.rigid_body_type(RigidBodyType::Dynamic)
					.build()
			);
		}
		
		Ok(application)
	}
	
	pub fn run(mut self) -> Result<(), ApplicationRunError> {
		let mut instant = Instant::now();
		
		while !self.input.quit_required {
			let mut delta_time = instant.elapsed();
			instant = Instant::now();
			
			if delta_time.as_millis() > 250 {
				println!("Can't keep up! Delta time: {:.2}s", delta_time.as_secs_f32());
				delta_time = Duration::from_millis(250);
			}
			
			self.input.reset();
			
			if let Some(window) = &mut self.window {
				window.pull_events(&mut self.input);
			}
			
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
				
				physics.step(Duration::from_millis(1000 / 140));
				
				for entity in self.entities.values() {
					entity.after_physics(physics);
				}
				
				physics.debug_draw();
			}
			
			for entity in self.entities.values() {
				entity.tick(delta_time, &self)?;
			}
			
			self.renderer.get_mut().begin_frame()?;
			
			let pov = self.pov
			              .get(&self).map(|e| *e.state().position)
			              .unwrap_or(Isometry3::identity());
			let detached_pov = self.detached_pov.get(&self).map(|e| *e.state().position);
			let hmd_pose = self.vr_poses.render[HMD as usize].device_to_absolute_tracking().clone();
			
			if let Some(eyes) = &mut self.eyes {
				eyes.set_hmd_pose(hmd_pose);
				eyes.load_background(pov, self.renderer.get_mut())?;
			}
			
			if let Some(window) = &mut self.window {
				match window.regen_swapchain(self.renderer.get_mut()) {
					Err(WindowSwapchainRegenError::NeedRetry) => {},
					result => result?,
				}
			}
			
			if let Some(eyes) = &mut self.eyes {
				self.renderer.get_mut().render(pov, &mut self.entities, eyes)?;
			
				if let Some(window) = &mut self.window {
					if let Some(detached_pov) = detached_pov {
						self.renderer.get_mut().render(detached_pov, &mut self.entities, window)?;
					} else {
						window.mirror_from(eyes.last_frame(), self.renderer.get_mut())?;
					}
				}
			} else if let Some(window) = &mut self.window {
				self.renderer.get_mut().render(pov, &mut self.entities, window)?;
			}
			
			if let Some(window) = &mut self.window {
				window.mark_dirty();
				self.renderer.get_mut().render(pov, &mut self.entities, window)?;
			}
			
			self.renderer.get_mut().end_frame()?;
			
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
			
			for entity in self.entities.values_mut() {
				if entity.add_new_components() {
					clean = false;
				}
			}
			
			if !clean {
				for entity in self.entities.values() {
					entity.setup_new_components(self)?;
				}
			}
		}
		
		Ok(())
	}
	
	fn cleanup_loop(&mut self) -> Result<(), ComponentError> {
		let mut clean = false;
		while !clean {
			clean = true;
			
			for entity in self.entities.values() {
				if entity.end_components(self)? {
					clean = false;
				}
			}
			
			if !clean {
				for entity in self.entities.values_mut() {
					entity.cleanup_ended_components();
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
		debug::set_flag("DebugBonesDraw", self.input.keyboard.toggle(Key::B));
		debug::set_flag("DebugCollidersDraw", self.input.keyboard.toggle(Key::C));
		debug::set_flag("DebugJointsDraw", self.input.keyboard.toggle(Key::J));
		debug::set_flag("DebugRigidBodiesDraw", self.input.keyboard.toggle(Key::M));
	}
}

#[derive(Debug, Error)]
pub enum ApplicationCreationError {
	#[error(display = "OpenvR unavailable. You can't use openvr background with --novr flag.")] OpenVRCameraInNoVR,
	#[error(display = "{}", _0)] RendererCreationError(#[error(source)] RendererError),
	#[error(display = "{}", _0)] VRError(#[error(source)] VRError),
	#[error(display = "{}", _0)] ObjLoadError(#[error(source)] ObjLoadError),
	#[error(display = "{}", _0)] EyesCreationError(#[error(source)] EyesCreationError),
	#[cfg(windows)] #[error(display = "{}", _0)] EscapiCameraError(#[error(source)] camera::EscapiCameraError),
	#[error(display = "{}", _0)] OpenVRCameraError(#[error(source)] camera::OpenVRCameraError),
	#[error(display = "{}", _0)] WindowCreationError(#[error(source)] WindowCreationError),
}

#[derive(Debug, Error)]
pub enum ApplicationRunError {
	#[error(display = "{}", _0)] RendererBeginFrameError(#[error(source)] RendererBeginFrameError),
	#[error(display = "{}", _0)] RendererRenderEyesError(#[error(source)] RendererRenderError<EyesRenderTargetError>),
	#[error(display = "{}", _0)] RendererRenderWindowError(#[error(source)] RendererRenderError<WindowRenderTargetError>),
	#[error(display = "{}", _0)] RendererEndFrameError(#[error(source)] RendererEndFrameError),
	#[error(display = "{}", _0)] EyesLoadBackgroundError(#[error(source)] EyesLoadBackgroundError),
	#[error(display = "{}", _0)] WindowSwapchainRegenError(#[error(source)] WindowSwapchainRegenError),
	#[error(display = "{}", _0)] WindowRenderError(#[error(source)] WindowMirrorFromError),
	#[error(display = "{}", _0)] ComponentError(ComponentError),
	#[error(display = "{}", _0)] CompositorError(#[error(source)] openvr::compositor::CompositorError),
}

impl From<ComponentError> for ApplicationRunError {
	fn from(err: ComponentError) -> Self {
		ApplicationRunError::ComponentError(err)
	}
}
