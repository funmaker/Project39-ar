use std::thread;
use std::sync::Arc;
use std::time::Instant;
use std::sync::mpsc;
use err_derive::Error;
use vulkano::buffer::{CpuBufferPool, BufferSlice, BufferAccess};
use vulkano::device::Queue;
use vulkano::image::{AttachmentImage, ImageCreationError, ImageUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CopyBufferImageError, BuildError, CommandBufferExecError, AutoCommandBuffer};
use vulkano::{format, OomError};
use vulkano::buffer::view::BufferViewCreationError;
use vulkano::memory::DeviceMemoryAllocError;

#[cfg(windows)] mod escapi;
mod opencv;
mod openvr;
mod dummy;

#[cfg(windows)] pub use self::escapi::{Escapi, EscapiCameraError};
pub use self::opencv::{OpenCV, OpenCVCameraError};
pub use self::openvr::{OpenVR, OpenVRCameraError};
pub use self::dummy::Dummy;

pub const CAPTURE_WIDTH: u32 = 1920;
pub const CAPTURE_HEIGHT: u32 = 960;
pub const CAPTURE_FPS: u64 = 60;
pub const CHUNK_SIZE: usize = CAPTURE_WIDTH as usize;

pub trait Camera: Send + Sized + 'static {
	fn capture(&mut self) -> Result<&[u8], CaptureError>;
	
	fn start(mut self, queue: Arc<Queue>)
		     -> Result<(Arc<AttachmentImage<format::B8G8R8A8Unorm>>, mpsc::Receiver<AutoCommandBuffer>), CameraStartError> {
		let target = AttachmentImage::with_usage(queue.device().clone(),
		                                         [CAPTURE_WIDTH, CAPTURE_HEIGHT],
		                                         format::B8G8R8A8Unorm,
		                                         ImageUsage { transfer_source: true,
			                                         transfer_destination: true,
			                                         ..ImageUsage::none() })?;
		let ret = target.clone();
		
		let (sender, receiver) = mpsc::sync_channel(1);
		
		thread::spawn(move || {
			match self.capture_loop(queue, target, sender) {
				Ok(()) => {},
				Err(CaptureLoopError::Quitting) => return,
				Err(err) => panic!("Error while capturing camera: {:?}", err),
			}
		});
		
		Ok((ret, receiver))
	}
	
	fn capture_loop(&mut self, queue: Arc<Queue>, target: Arc<AttachmentImage<format::B8G8R8A8Unorm>>, sender: mpsc::SyncSender<AutoCommandBuffer>) -> Result<(), CaptureLoopError> {
		let buffer = CpuBufferPool::upload(queue.device().clone());
		let mut last_capture = Instant::now();
		
		loop {
			let frame = match self.capture() {
				Ok(frame) => frame,
				Err(CaptureError::Timeout) => continue,
				Err(err) => return Err(err.into()),
			};
			
			// println!("{} FPS\t{}", 1.0 / last_capture.elapsed().as_secs_f32(), frame.len());
			last_capture = Instant::now();
			
			let sub_buffer = buffer.chunk(
				frame.chunks_exact(CHUNK_SIZE)
				     .map(|c| unsafe { *(c.as_ptr() as *const [u8; CHUNK_SIZE]) })
			)?;
			let sub_slice: BufferSlice<[u8], _> = unsafe { sub_buffer.into_buffer_slice().reinterpret() };
			
			let mut builder  = AutoCommandBufferBuilder::new(queue.device().clone(), queue.family())?;
			builder.copy_buffer_to_image(sub_slice, target.clone())?;
			let command_buffer = builder.build()?;
			
			sender.send(command_buffer).or(Err(CaptureLoopError::Quitting))?;
		}
	}
}

#[derive(Debug, Error)]
pub enum CaptureError {
	#[error(display = "Timeout while waiting for a frame")] Timeout,
	#[error(display = "{}", _0)] Other(Box<dyn std::error::Error>),
}

#[derive(Debug, Error)]
pub enum CameraStartError {
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] ImageCreationError),
}

#[derive(Debug, Error)]
pub enum CaptureLoopError {
	#[error(display = "Quitting")] Quitting,
	#[error(display = "{}", _0)] CaptureError(#[error(source)] CaptureError),
	#[error(display = "{}", _0)] OomError(#[error(source)] OomError),
	#[error(display = "{}", _0)] CopyBufferImageError(#[error(source)] CopyBufferImageError),
	#[error(display = "{}", _0)] BuildError(#[error(source)] BuildError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] CommandBufferExecError),
	#[error(display = "{}", _0)] BufferViewCreationError(#[error(source)] BufferViewCreationError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] DeviceMemoryAllocError),
}
