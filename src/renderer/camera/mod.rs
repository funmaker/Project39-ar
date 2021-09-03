use std::thread;
use std::sync::Arc;
use std::time::Instant;
use std::sync::mpsc;
use err_derive::Error;
use vulkano::{memory};
use vulkano::command_buffer::{self, AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, CommandBufferUsage};
use vulkano::buffer::{self, CpuBufferPool, BufferSlice, BufferAccess};
use vulkano::image::{AttachmentImage, ImageUsage};
use vulkano::device::{Queue};
use vulkano::format::Format;

#[cfg(windows)] mod escapi;
mod opencv;
mod openvr;
mod dummy;

#[cfg(windows)] pub use self::escapi::{Escapi, EscapiCameraError};
pub use self::opencv::{OpenCV, OpenCVCameraError};
pub use self::openvr::{OpenVR, OpenVRCameraError};
pub use self::dummy::Dummy;
use crate::debug;
use crate::math::Isometry3;

pub const CAPTURE_WIDTH: u32 = 1920;
pub const CAPTURE_HEIGHT: u32 = 960;
pub const CAPTURE_FPS: u64 = 140;
pub const CHUNK_SIZE: usize = CAPTURE_WIDTH as usize;

pub trait Camera: Send + Sized + 'static {
	fn capture(&mut self) -> Result<(&[u8], Option<Isometry3>), CameraCaptureError>;
	
	fn start(mut self, queue: Arc<Queue>)
		     -> Result<(Arc<AttachmentImage>, mpsc::Receiver<(PrimaryAutoCommandBuffer, Option<Isometry3>)>), CameraStartError> {
		let target = AttachmentImage::with_usage(queue.device().clone(),
		                                         [CAPTURE_WIDTH, CAPTURE_HEIGHT],
		                                         Format::B8G8R8A8_UNORM,
		                                         ImageUsage { sampled: true,
			                                         transfer_destination: true,
			                                         ..ImageUsage::none() })?;
		let ret = target.clone();
		
		let (sender, receiver) = mpsc::sync_channel(1);
		
		thread::spawn(move || {
			match self.capture_loop(queue, target, sender) {
				Ok(()) => {},
				Err(CaptureLoopError::Quitting) => return,
				Err(err) => panic!("Error while capturing background: {:?}", err),
			}
		});
		
		Ok((ret, receiver))
	}
	
	fn capture_loop(&mut self, queue: Arc<Queue>, target: Arc<AttachmentImage>, sender: mpsc::SyncSender<(PrimaryAutoCommandBuffer, Option<Isometry3>)>) -> Result<(), CaptureLoopError> {
		let buffer = CpuBufferPool::upload(queue.device().clone());
		let mut last_capture = Instant::now();
		
		loop {
			let frame = match self.capture() {
				Ok(frame) => frame,
				Err(CameraCaptureError::Timeout) => continue,
				Err(err) => return Err(err.into()),
			};
			
			debug::set_flag("CAMERA_FPS", 1.0 / last_capture.elapsed().as_secs_f32());
			last_capture = Instant::now();
			
			let sub_buffer = buffer.chunk(
				frame.0
				     .chunks_exact(CHUNK_SIZE)
				     .map(|c| unsafe { *(c.as_ptr() as *const [u8; CHUNK_SIZE]) })
			)?;
			let sub_slice: BufferSlice<[u8], _> = unsafe { sub_buffer.into_buffer_slice().reinterpret() };
			
			let mut builder  = AutoCommandBufferBuilder::primary(queue.device().clone(), queue.family(), CommandBufferUsage::OneTimeSubmit)?;
			builder.copy_buffer_to_image(sub_slice, target.clone())?;
			let command_buffer = builder.build()?;
			
			sender.send((command_buffer, frame.1)).or(Err(CaptureLoopError::Quitting))?;
		}
	}
}

#[derive(Debug, Error)]
pub enum CameraCaptureError {
	#[error(display = "Timeout while waiting for a frame")] Timeout,
	#[error(display = "{}", _0)] Other(Box<dyn std::error::Error>),
}

#[derive(Debug, Error)]
pub enum CameraStartError {
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] vulkano::image::ImageCreationError),
}

#[derive(Debug, Error)]
pub enum CaptureLoopError {
	#[error(display = "Quitting")] Quitting,
	#[error(display = "{}", _0)] CaptureError(#[error(source)] CameraCaptureError),
	#[error(display = "{}", _0)] OomError(#[error(source)] vulkano::OomError),
	#[error(display = "{}", _0)] CopyBufferImageError(#[error(source)] command_buffer::CopyBufferImageError),
	#[error(display = "{}", _0)] BuildError(#[error(source)] command_buffer::BuildError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] command_buffer::CommandBufferExecError),
	#[error(display = "{}", _0)] BufferViewCreationError(#[error(source)] buffer::view::BufferViewCreationError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
}
