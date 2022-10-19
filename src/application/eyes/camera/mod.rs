use std::thread;
use std::sync::Arc;
use std::sync::mpsc;
use err_derive::Error;
use vulkano::{memory, command_buffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, CommandBufferUsage};
use vulkano::buffer::CpuBufferPool;
use vulkano::image::{AttachmentImage, ImageUsage};
use vulkano::device::Queue;
use vulkano::format::Format;

#[cfg(windows)] mod escapi;
#[cfg(feature = "opencv-camera")] mod opencv;
mod openvr;
mod dummy;

#[cfg(windows)] pub use self::escapi::{Escapi, EscapiCameraError};
#[cfg(feature = "opencv-camera")] pub use self::opencv::{OpenCV, OpenCVCameraError};
pub use self::openvr::{OpenVR, OpenVRCameraError};
pub use self::dummy::Dummy;
use crate::debug;
use crate::math::Isometry3;
use crate::utils::FpsCounter;

pub const CAPTURE_WIDTH: u32 = 1920;
pub const CAPTURE_HEIGHT: u32 = 960;
pub const CAPTURE_FPS: u64 = 140;
pub const CHUNK_SIZE: usize = CAPTURE_WIDTH as usize;

pub trait Camera: Send + 'static {
	fn capture(&mut self) -> Result<(&[u8], Option<Isometry3>), CameraCaptureError>;
	
	fn start(mut self: Box<Self>, queue: Arc<Queue>)
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
		let mut fps_counter = FpsCounter::<20>::new();
		
		loop {
			let frame = match self.capture() {
				Ok(frame) => frame,
				Err(CameraCaptureError::Timeout) => continue,
				Err(err) => return Err(err.into()),
			};
			
			fps_counter.tick();
			debug::set_flag("CAMERA_FPS", fps_counter.fps());
			
			let sub_buffer = buffer.chunk(frame.0.array_chunks::<CHUNK_SIZE>().copied())?;
			
			let mut builder  = AutoCommandBufferBuilder::primary(queue.device().clone(), queue.family(), CommandBufferUsage::OneTimeSubmit)?;
			builder.copy_buffer_to_image(sub_buffer, target.clone())?;
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
	#[error(display = "{}", _0)] DeviceMemoryAllocationError(#[error(source)] memory::DeviceMemoryAllocationError),
}