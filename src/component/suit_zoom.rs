use std::cell::Cell;
use std::time::{Duration, Instant};

use crate::application::{Entity, Application, Key, Hand};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::debug;
use crate::math::{Color, PI, Point3, Similarity3, Translation3};

#[derive(ComponentBase)]
pub struct SuitZoom {
	#[inner] inner: ComponentInner,
	fov_scale: Cell<f32>,
	zoom: Cell<(bool, Instant)>,
}

impl SuitZoom {
	pub fn new() -> Self {
		SuitZoom {
			inner: ComponentInner::new(),
			fov_scale: Cell::new(1.0),
			zoom: Cell::new((false, Instant::now())),
		}
	}
}

impl Component for SuitZoom {
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> {
		let mut fov_scale = self.fov_scale.get();
		
		if application.input.keyboard.pressed(Key::Equals) { fov_scale += 1.0 * delta_time.as_secs_f32() }
		if application.input.keyboard.pressed(Key::Minus)  { fov_scale -= 1.0 * delta_time.as_secs_f32() }
		
		if let Some(controller) = application.input.controller(Hand::Left) {
			fov_scale += controller.axis(0) * delta_time.as_secs_f32();
			
			if controller.down(1) {
				self.zoom.set((true, Instant::now()));
			} else if controller.up(1) {
				self.zoom.set((false, Instant::now()));
			}
		}
		
		if application.input.keyboard.down(Key::Z) {
			self.zoom.set((true, Instant::now()));
		} else if application.input.keyboard.up(Key::Z) {
			self.zoom.set((false, Instant::now()));
		}
		
		// Based on Source Engine :^)
		
		fn simple_spline(val: f32) -> f32 {
			let val2 = val * val;
			3.0 * val2 - 2.0 * val2 * val
		}
		
		fn simple_spline_remap(val: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
			if a == b {
				if val >= b { b } else { c }
			} else {
				let c_val = (val - a) / (b - a);
				
				c + (d - c) * simple_spline(c_val)
			}
		}
		
		let default_fov = debug::get_flag("FOV").unwrap_or(130.0 / 180.0 * PI);
		let normal_fov = default_fov * fov_scale;
		let target_fov = 25.0 / 180.0 * PI;
		let (is_zooming, changed) = self.zoom.get();
		let elapsed = changed.elapsed().as_secs_f32();
		
		let final_fov = if is_zooming && elapsed > 0.4 {
			target_fov
		} else if is_zooming {
			simple_spline_remap(elapsed / 0.4, 0.0, 1.0, normal_fov, target_fov)
		} else if !is_zooming && elapsed > 0.2 {
			normal_fov
		} else {
			simple_spline_remap(elapsed / 0.2, 0.0, 1.0, target_fov, normal_fov)
		};
		
		let final_scale = (final_fov / 2.0).tan() / (default_fov / 2.0).tan();
		
		self.fov_scale.set(fov_scale);
		application.fov_scale.set(final_scale);
		
		let mut scale = (elapsed / 0.4).clamp(0.0, 1.0);
		let alpha;
		
		if is_zooming {
			alpha = scale;
		} else {
			if scale > 1.0 {
				return Ok(())
			}
			
			alpha = (1.0 - scale) * 0.25;
			scale = 1.0 - (scale * 0.5);
		}
		
		let color = Color::new(1.0, 0.863, 0.0, 1.0).opactiy(alpha * 94.0 / 255.0);
		let transform = Similarity3::from_isometry(
			entity.state().position * Translation3::new(0.0, 0.0, -1.0),
			0.001
		);
		
		fn at(n: i32, steps: i32) -> Point3 {
			let angle = (n as f32 / steps as f32) * 2.0 * PI;
			point!(angle.sin(), angle.cos(), 0.0)
		}
			
		for i in 0..48 {
			let from = at(i, 48) * 66.0 * scale;
			let to = at(i + 1, 48) * 66.0 * scale;
			debug::draw_line(transform * from, transform * to, 4.0, color);
		}
		
		for i in 0..64 {
			let from = at(i, 64) * 74.0 * scale;
			let to = at(i + 1, 64) * 74.0 * scale;
			debug::draw_line(transform * from, transform * to, 4.0, color);
		}
		
		let height = 2.0;
		let gap = 16.0 * scale.max(0.1);
		for dash in 2..100 {
			let x = 2.0 - gap * dash as f32;
			debug::draw_line(transform * point!( x, -height, 0.0), transform * point!( x, height, 0.0), 4.0, color);
			debug::draw_line(transform * point!(-x, -height, 0.0), transform * point!(-x, height, 0.0), 4.0, color);
		}
		
		Ok(())
	}
}
