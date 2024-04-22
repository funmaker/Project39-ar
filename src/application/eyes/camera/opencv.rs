use anyhow::Result;
use thiserror::Error;
use opencv::{videoio, imgproc, core};
use opencv::prelude::*;

use crate::math::Isometry3;
use super::{CAPTURE_WIDTH, CAPTURE_HEIGHT, CAPTURE_FPS, Camera, CameraCaptureTimeout};


pub const CAPTURE_INDEX: i32 = 0;

pub struct OpenCV {
	inner: videoio::VideoCapture,
	frame: Mat,
	frame_rgba: Mat,
}

impl OpenCV {
	pub fn new() -> Result<OpenCV> {
		let mut inner = videoio::VideoCapture::new(CAPTURE_INDEX, videoio::CAP_ANY)?;
		
		if !videoio::VideoCapture::is_opened(&inner)? {
			return Err(OpenCVCameraError::CameraOpenError.into());
		}
		
		inner.set(videoio::CAP_PROP_FRAME_WIDTH, CAPTURE_WIDTH as f64)?;
		inner.set(videoio::CAP_PROP_FRAME_HEIGHT, CAPTURE_HEIGHT as f64)?;
		inner.set(videoio::CAP_PROP_FPS, CAPTURE_FPS as f64)?;
		
		dprintln!("Camera {}: {}x{}", CAPTURE_INDEX, inner.get(videoio::CAP_PROP_FRAME_WIDTH)?, inner.get(videoio::CAP_PROP_FRAME_HEIGHT)?);
		
		Ok(OpenCV{
			inner,
			frame: Mat::default(),
			frame_rgba: Mat::default(),
		})
	}
}

impl Camera for OpenCV {
	fn capture(&mut self) -> Result<(&[u8])> {
		if !self.inner.read(&mut self.frame)? {
			return Err(CameraCaptureTimeout.into());
		}
		
		imgproc::cvt_color(&self.frame, &mut self.frame_rgba, imgproc::COLOR_RGB2RGBA, 4)?;
		let (_, slice, _) = unsafe { self.frame_rgba.data_typed::<core::Vec4b>()?.align_to() };
		
		return Ok((slice, None));
	}
}

#[derive(Debug, Error)]
pub enum OpenCVCameraError {
	#[error("Failed to open background")] CameraOpenError,
}
