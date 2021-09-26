#![allow(dead_code)]

use std::time::{Instant, Duration};
use std::thread;
use std::sync::Arc;
use image::RgbaImage;

use super::{CAPTURE_WIDTH, CAPTURE_HEIGHT, CAPTURE_FPS, Camera, CameraCaptureError};
use crate::math::Isometry3;
use crate::debug;

lazy_static!(
	static ref FRAME: Vec<u8> = [  0,   0,   0,  39].iter()
	                                                .copied()
	                                                .cycle()
	                                                .take((CAPTURE_WIDTH * CAPTURE_HEIGHT * 4) as usize)
	                                                .collect();
);

pub struct Dummy {
	last_frame: Instant,
	camera_override: Option<Arc<RgbaImage>>,
}

impl Dummy {
	pub fn new() -> Dummy {
		Dummy {
			last_frame: Instant::now(),
			camera_override: None,
		}
	}
}

impl Camera for Dummy {
	fn capture(&mut self) -> Result<(&[u8], Option<Isometry3>), CameraCaptureError> {
		let next_frame = self.last_frame + Duration::from_millis(1000 / CAPTURE_FPS);
		
		if let Some(sleep_duration) = next_frame.checked_duration_since(Instant::now()) {
			thread::sleep(sleep_duration);
		}
		
		self.last_frame = Instant::now();
		
		self.camera_override = debug::get_flag("camera_override");
		
		if let Some(image) = self.camera_override.as_ref() {
			Ok((image, None))
		} else {
			Ok((FRAME.as_slice(), None))
		}
	}
}
