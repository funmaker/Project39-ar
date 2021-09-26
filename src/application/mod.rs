use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;
use std::fs::read_dir;
use std::path::PathBuf;
use err_derive::Error;
use openvr::TrackedControllerRole;
use openvr::compositor::WaitPoses;
use chrono::{DateTime, Utc, NaiveDateTime};
use image::{RgbaImage, DynamicImage};

pub use entity::{Entity, EntityRef};
pub use vr::{VR, VRError};

use crate::component::{Component, ComponentError, ComponentRef};
use crate::utils::default_wait_poses;
use crate::config::{self, CameraAPI, CameraConfig};
use crate::debug;
use crate::math::{Isometry3, Point2, Color, Point3, Rot3};
use crate::steamvr_config::{load_steamvr_config, CameraConfigLoadError};
use crate::renderer::{Renderer, RendererError, RendererRenderError, RendererBackgroundError};
use crate::renderer::camera::{self, OpenCVCameraError, OpenVRCameraError};
use crate::renderer::window::{Window, WindowCreationError};
use crate::component::model::mmd::MMDModelLoadError;
use crate::component::model::ModelError;
use crate::component::model::simple::SimpleModelLoadError;
use crate::component::mirror::Mirror;

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
	mirror: ComponentRef<Mirror>,
}

pub struct Dump {
	time: DateTime<Utc>,
	camera: Arc<RgbaImage>,
	mirror: Arc<DynamicImage>,
	config: CameraConfig,
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
			mirror: ComponentRef::null(),
		};
		
		let mirror = Entity::new(
			"Mirror",
			Point3::new(0.0, 0.0, 100.0),
			Rot3::identity(),
			None,
		);
		
		application.mirror = mirror.add_component(Mirror::new(application.renderer.get_mut()));
		application.add_entity(mirror);
		
		Ok(application)
	}
	
	pub fn run(mut self) -> Result<(), ApplicationRunError> {
		let mut instant = Instant::now();
		let mut vr_buttons = 0;
		
		let mut cur_dump = 0_usize;
		let mut mirror = false;
		let mut dumps = vec![];
		let mut dump_changed = true;
		let dumps_path = PathBuf::from("dumps");
		let dumps_dir = read_dir(&dumps_path)?;
		
		for dump in dumps_dir {
			let dump = dump?;
			
			let time = if let Ok(timestamp) = dump.file_name().to_string_lossy().parse() {
				DateTime::from_utc(NaiveDateTime::from_timestamp(timestamp, 0), Utc)
			} else {
				continue;
			};
			
			let path = dump.path();
			let mut camera = image::open(path.join("camera.png"))?.into_rgba8();
			let mut mirror = image::open(path.join("mirror.png"))?.into_rgba8();
			let config = load_steamvr_config(path.join("config.json"))?;
			
			for pixel in mirror.pixels_mut() {
				pixel.0[3] = 255;
			}
			
			for pixel in camera.pixels_mut() {
				pixel.0.swap(0, 2);
			}
			
			dumps.push(Dump {
				time,
				camera: Arc::new(camera),
				mirror: Arc::new(DynamicImage::ImageRgba8(mirror)),
				config,
			});
		}
		
		if dumps.is_empty() {
			panic!("No dumps to examine");
		}
		
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
			
			if debug::get_flag_or_default("KeyA") {
				cur_dump = cur_dump.checked_sub(1).unwrap_or(dumps.len() - 1);
				debug::set_flag("KeyA", false);
				dump_changed = true;
			}
			
			if debug::get_flag_or_default("KeyD") {
				cur_dump += 1;
				if cur_dump >= dumps.len() {
					cur_dump = 0;
				}
				debug::set_flag("KeyD", false);
				dump_changed = true;
			}
			
			if debug::get_flag_or_default("KeyE") {
				mirror = !mirror;
				
				self.mirror.get(&self).unwrap().set_enabled(mirror);
				debug::set_flag("KeyE", false);
			}
			
			let dump = &dumps[cur_dump];
			
			if dump_changed {
				debug::set_flag("camera_override", dump.camera.clone());
				config::rcu(|config| config.camera = dump.config.clone());
				self.renderer.get_mut().recreate_background()?;
				self.mirror.get(&self).unwrap().set_image(dump.mirror.clone(), &*self.renderer.borrow());
				
				dump_changed = false;
			}
			
			let pov = self.camera_entity.get(&self).map(|e| e.state().position)
			                            .unwrap_or(Isometry3::identity());
			
			let text = format!("({}/{}) {}", cur_dump + 1, dumps.len(), dump.time.to_rfc2822());
			debug::draw_text(text, Point2::new(-1.0, 1.0), debug::DebugOffset::top_right(16.0, -16.0), 64.0, Color::green());
			if mirror {
				debug::draw_text("Room View", Point2::new(-1.0, 1.0), debug::DebugOffset::top_right(16.0, -96.0), 64.0, Color::cyan());
			} else {
				debug::draw_text("Reconstruction", Point2::new(-1.0, 1.0), debug::DebugOffset::top_right(16.0, -96.0), 64.0, Color::green());
			}
			
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
	#[error(display = "{}", _0)] CameraConfigLoadError(#[error(source)] CameraConfigLoadError),
	#[error(display = "{}", _0)] RendererBackgroundError(#[error(source)] RendererBackgroundError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] CompositorError(#[error(source)] openvr::compositor::CompositorError),
	#[error(display = "{}", _0)] TrackedPropertyError(#[error(source)] openvr::system::TrackedPropertyError),
	#[error(display = "{}", _0)] RenderModelError(#[error(source)] openvr::render_models::Error),
	#[error(display = "{}", _0)] ObjError(#[error(source)] obj::ObjError),
	#[error(display = "{}", _0)] IOError(#[error(source)] std::io::Error),
}

impl From<ComponentError> for ApplicationRunError {
	fn from(err: ComponentError) -> Self {
		ApplicationRunError::ComponentError(err)
	}
}
