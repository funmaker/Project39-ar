#![allow(dead_code)]

use err_derive::Error;

use crate::math::Isometry3;
use super::{CAPTURE_WIDTH, CAPTURE_HEIGHT, CAPTURE_FPS, Camera, CameraCaptureError};


pub const CAPTURE_INDEX: usize = 0;

pub struct Escapi {
	inner: escapi::Device,
}

impl Escapi {
	pub fn new() -> Result<Escapi, EscapiCameraError> {
		let inner = escapi::init(CAPTURE_INDEX, CAPTURE_WIDTH, CAPTURE_HEIGHT, CAPTURE_FPS)?;
		
		dprintln!("Camera {}: {}x{}", inner.name(), inner.capture_width(), inner.capture_height());
		
		Ok(Escapi {
			inner,
		})
	}
}

impl Camera for Escapi {
	fn capture(&mut self) -> Result<(&[u8], Option<Isometry3>), CameraCaptureError> {
		match self.inner.capture() {
			Ok(frame) => Ok((frame, None)),
			Err(escapi::Error::CaptureTimeout) => Err(CameraCaptureError::Timeout),
			Err(err) => Err(CameraCaptureError::Other(err.into())),
		}
	}
}

#[derive(Debug, Error)]
pub enum EscapiCameraError {
	#[error(display = "{}", _0)] EscapiError(#[error(source)] escapi::Error),
}
