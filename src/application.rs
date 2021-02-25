use std::time::{Instant, Duration};
use err_derive::Error;

pub mod entity;

use crate::renderer::{Renderer, RendererError, RendererRenderError};
use crate::renderer::window::{Window, WindowCreationError};
use crate::renderer::model::{ModelError, mmd::MMDModelLoadError, simple::SimpleModelLoadError, Model, MMDModel};
use crate::math::{Vec3, Rot3, Isometry3, Translation3, Point3};
use crate::debug;
pub use entity::Entity;

pub struct Application {
	renderer: Renderer,
	window: Window,
	scene: Vec<Entity>,
	model: MMDModel<u16>,
}

type FakePose = (Translation3, (f32, f32));

impl Application {
	pub fn new(device: Option<usize>) -> Result<Application, ApplicationCreationError> {
		let mut renderer = Renderer::new(device)?;
		
		let window = Window::new(&renderer)?;
		
		let scene: Vec<Entity> = Vec::new();
		
		let model = MMDModel::from_pmx("models/YYB式初音ミクCrude Hair/YYB式初音ミクCrude Hair.pmx", &mut renderer)?;
		// let model = SimpleModel::from_obj("models/cube/cube", &mut renderer)?;
		
		Ok(Application {
			renderer,
			window,
			scene,
			model,
		})
	}
	
	pub fn run(mut self) -> Result<(), ApplicationRunError> {
		let mut fake_pose: FakePose = (Translation3::identity(), (0.0, 0.0));
		
		let tests = [1, 5, 10, 20, 50, 100];
		let mut results = vec![];
		
		for count in &tests {
			let result = self.run_test(&mut fake_pose, *count)?;
			
			results.push((*count, result));
		}
		
		println!("\nResults:");
		
		for (count, result) in results {
			println!("{}: {}", count, result);
		}
		
		Ok(())
	}
	
	pub fn run_test(&mut self, fake_pose: &mut FakePose, count: usize) -> Result<usize, ApplicationRunError> {
		self.scene.clear();
		
		println!("\nTesting {} models", count);
		println!("Setting up...");
		
		for id in 0..count {
			let x = (id % 10) as f32 - count.min(10) as f32 * 0.5;
			let z = -5.0 - (id / 10) as f32 * 0.5;
			
			self.scene.push(Entity::new(
				format!("{}", id),
				self.model.try_clone(&mut self.renderer)?,
				Point3::new(x, -1.0, z),
				Rot3::from_euler_angles(0.0, std::f32::consts::PI, 0.0),
			));
		}
		
		println!("Warming up...");
		
		self.run_for(fake_pose, Duration::from_secs(1))?;
		
		println!("Test start");
		
		let result = self.run_for(fake_pose, Duration::from_secs(10))?;
		
		println!("Done: {} frames", result);
		
		Ok(result)
	}
	
	pub fn run_for(&mut self, fake_pose: &mut FakePose, duration: Duration) -> Result<usize, ApplicationRunError> {
		let start = Instant::now();
		let mut instant = Instant::now();
		let mut frames = 0;
		
		while !self.window.quit_required && start.elapsed() < duration {
			self.window.pull_events();
			
			let delta_time = instant.elapsed();
			instant = Instant::now();
			
			let pose = self.handle_fake_pose(fake_pose, delta_time);
			
			for entity in self.scene.iter_mut() {
				entity.tick(delta_time);
				
				for morph in entity.morphs.iter_mut() {
					*morph = 0.0;
				}
				
				let morphs = entity.morphs.len();
				for _ in 0 .. 10 {
					entity.morphs[rand::random::<usize>() % morphs] = rand::random();
				}
			}
			
			self.renderer.render(pose, &mut self.scene, &mut self.window)?;
			frames += 1;
		}
		
		Ok(frames)
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
		let disp = rot * Vec3::new(x, 0.0, z) * dist + Vec3::y() * y;
		
		position.vector += disp;
		
		Isometry3::from_parts(position.clone(), rot)
	}
}

#[derive(Debug, Error)]
pub enum ApplicationCreationError {
	#[error(display = "{}", _0)] RendererCreationError(#[error(source)] RendererError),
	#[error(display = "{}", _0)] MMDModelLoadError(#[error(source)] MMDModelLoadError),
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] SimpleModelLoadError(#[error(source)] SimpleModelLoadError),
	#[error(display = "{}", _0)] WindowCreationError(#[error(source)] WindowCreationError),
}

#[derive(Debug, Error)]
pub enum ApplicationRunError {
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] SimpleModelLoadError(#[error(source)] SimpleModelLoadError),
	#[error(display = "{}", _0)] RendererRenderError(#[error(source)] RendererRenderError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
	#[error(display = "{}", _0)] ObjError(#[error(source)] obj::ObjError),
}

