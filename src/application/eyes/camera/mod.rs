use std::thread;
use std::sync::{Arc, mpsc};
use err_derive::Error;
use vulkano::{memory, command_buffer};
use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, CommandBufferUsage, CopyBufferToImageInfo};
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::device::Queue;
use vulkano::format::Format;
use vulkano::image::{AttachmentImage, ImageUsage};
use vulkano::memory::allocator::StandardMemoryAllocator;

mod dummy;
#[cfg(windows)] mod escapi;
#[cfg(feature = "opencv-camera")] mod opencv;
mod openvr;

use crate::debug;
use crate::math::Isometry3;
use crate::utils::{FpsCounter, SubbufferAllocatorEx, SubbufferAllocatorExError};
pub use self::dummy::Dummy;
#[cfg(windows)] pub use self::escapi::{Escapi, EscapiCameraError};
#[cfg(feature = "opencv-camera")] pub use self::opencv::{OpenCV, OpenCVCameraError};
pub use self::openvr::{OpenVR, OpenVRCameraError};


pub const CAPTURE_WIDTH: u32 = 1920;
pub const CAPTURE_HEIGHT: u32 = 960;
pub const CAPTURE_FPS: u64 = 140;
pub const CHUNK_SIZE: usize = CAPTURE_WIDTH as usize;

pub trait Camera: Send + 'static {
	fn capture(&mut self) -> Result<(&[u8], Option<Isometry3>), CameraCaptureError>;
	
	fn start(mut self: Box<Self>, queue: Arc<Queue>, memory_allocator: Arc<StandardMemoryAllocator>, command_buffer_allocator: Arc<StandardCommandBufferAllocator>)
		     -> Result<(Arc<AttachmentImage>, mpsc::Receiver<(PrimaryAutoCommandBuffer, Option<Isometry3>)>), CameraStartError> {
		let target = AttachmentImage::with_usage(&*memory_allocator,
		                                         [CAPTURE_WIDTH, CAPTURE_HEIGHT],
		                                         Format::B8G8R8A8_SRGB,
		                                         ImageUsage::SAMPLED
			                                         | ImageUsage::TRANSFER_DST)?;
		let ret = target.clone();
		
		let (sender, receiver) = mpsc::sync_channel(1);
		
		thread::spawn(move || {
			match self.capture_loop(queue, memory_allocator, command_buffer_allocator, target, sender) {
				Ok(()) => {},
				Err(CaptureLoopError::Quitting) => return,
				Err(err) => panic!("Error while capturing background: {:?}", err),
			}
		});
		
		Ok((ret, receiver))
	}
	
	fn capture_loop(&mut self, queue: Arc<Queue>, memory_allocator: Arc<StandardMemoryAllocator>, command_buffer_allocator: Arc<StandardCommandBufferAllocator>, target: Arc<AttachmentImage>, sender: mpsc::SyncSender<(PrimaryAutoCommandBuffer, Option<Isometry3>)>) -> Result<(), CaptureLoopError> {
		let allocator = SubbufferAllocator::new(memory_allocator, SubbufferAllocatorCreateInfo::default());
		let mut fps_counter = FpsCounter::<20>::new();
		
		loop {
			let frame = match self.capture() {
				Ok(frame) => frame,
				Err(CameraCaptureError::Timeout) => continue,
				Err(err) => return Err(err.into()),
			};
			
			fps_counter.tick();
			debug::set_flag("CAMERA_FPS", fps_counter.fps());
			
			let sub_buffer = allocator.from_iter(frame.0.array_chunks::<CHUNK_SIZE>().copied())?;
			
			let mut builder  = AutoCommandBufferBuilder::primary(&*command_buffer_allocator, queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit)?;
			builder.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(sub_buffer, target.clone()))?;
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
	#[error(display = "{}", _0)] ImageError(#[error(source)] vulkano::image::ImageError),
}

#[derive(Debug, Error)]
pub enum CaptureLoopError {
	#[error(display = "Quitting")] Quitting,
	#[error(display = "{}", _0)] CaptureError(#[error(source)] CameraCaptureError),
	#[error(display = "{}", _0)] SubbufferAllocatorExError(#[error(source)] SubbufferAllocatorExError),
	#[error(display = "{}", _0)] CopyError(#[error(source)] command_buffer::CopyError),
	#[error(display = "{}", _0)] CommandBufferBeginError(#[error(source)] command_buffer::CommandBufferBeginError),
	#[error(display = "{}", _0)] BuildError(#[error(source)] command_buffer::BuildError),
}
