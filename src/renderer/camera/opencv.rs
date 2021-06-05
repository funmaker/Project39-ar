use err_derive::Error;
use opencv::prelude::*;
use opencv::{ videoio, imgproc, core };

use super::{CAPTURE_WIDTH, CAPTURE_HEIGHT, CAPTURE_FPS, Camera, CameraCaptureError};

pub const CAPTURE_INDEX: i32 = 0;

pub struct OpenCV {
	inner: videoio::VideoCapture,
	frame: Mat,
	frame_rgba: Mat,
}

impl OpenCV {
	pub fn new() -> Result<OpenCV, OpenCVCameraError> {
		let mut inner = videoio::VideoCapture::new(CAPTURE_INDEX, videoio::CAP_ANY)?;
		
		if !videoio::VideoCapture::is_opened(&inner)? {
			return Err(OpenCVCameraError::CameraOpenError);
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
	fn capture(&mut self) -> Result<&[u8], CameraCaptureError> {
		if !self.inner.read(&mut self.frame)? {
			return Err(CameraCaptureError::Timeout);
		}
		
		imgproc::cvt_color(&self.frame, &mut self.frame_rgba, imgproc::COLOR_RGB2RGBA, 4)?;
		let (_, slice, _) = unsafe { self.frame_rgba.data_typed::<core::Vec4b>()?.align_to() };
		
		return Ok(slice);
	}
}

#[derive(Debug, Error)]
pub enum OpenCVCameraError {
	#[error(display = "Failed to open camera")] CameraOpenError,
	#[error(display = "{}", _0)] OpenCVError(#[error(source)] opencv::Error),
}

impl From<opencv::Error> for CameraCaptureError {
	fn from(err: opencv::Error) -> Self {
		CameraCaptureError::Other(err.into())
	}
}
