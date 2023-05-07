#![allow(dead_code)]

use std::thread;
use std::time::{Instant, Duration};

use crate::math::Isometry3;
use super::{CAPTURE_WIDTH, CAPTURE_HEIGHT, CAPTURE_FPS, Camera, CameraCaptureError};


lazy_static!(
	static ref FRAME: Vec<u8> = [ 57,  45,  45, 255].iter()
	                                                .copied()
	                                                .cycle()
	                                                .take((CAPTURE_WIDTH * CAPTURE_HEIGHT * 4) as usize)
	                                                .collect();
);

pub struct Dummy {
	last_frame: Instant,
}

impl Dummy {
	pub fn new() -> Dummy {
		Dummy {
			last_frame: Instant::now(),
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
		
		Ok((FRAME.as_slice(), None))
	}
}
