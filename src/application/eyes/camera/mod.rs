use std::thread;
use std::sync::{Arc, mpsc};
use anyhow::Result;
use thiserror::Error;
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
use crate::utils::{FpsCounter, SubbufferAllocatorEx};
pub use self::dummy::Dummy;
#[cfg(windows)] pub use self::escapi::Escapi;
#[cfg(feature = "opencv-camera")] pub use self::opencv::OpenCV;
pub use self::openvr::OpenVR;


pub const CAPTURE_WIDTH: u32 = 1920;
pub const CAPTURE_HEIGHT: u32 = 960;
pub const CAPTURE_FPS: u64 = 140;
pub const CHUNK_SIZE: usize = CAPTURE_WIDTH as usize;

pub trait Camera: Send + 'static {
	fn capture(&mut self) -> Result<(&[u8], Option<Isometry3>)>;
	
	fn start(mut self: Box<Self>, queue: Arc<Queue>, memory_allocator: Arc<StandardMemoryAllocator>, command_buffer_allocator: Arc<StandardCommandBufferAllocator>)
		     -> Result<(Arc<AttachmentImage>, mpsc::Receiver<(PrimaryAutoCommandBuffer, Option<Isometry3>)>)> {
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
				Err(err) if err.is::<CaptureLoopQuitting>() => return,
				Err(err) => panic!("Error while capturing background: {:?}", err),
			}
		});
		
		Ok((ret, receiver))
	}
	
	fn capture_loop(&mut self, queue: Arc<Queue>, memory_allocator: Arc<StandardMemoryAllocator>, command_buffer_allocator: Arc<StandardCommandBufferAllocator>, target: Arc<AttachmentImage>, sender: mpsc::SyncSender<(PrimaryAutoCommandBuffer, Option<Isometry3>)>) -> Result<()> {
		let allocator = SubbufferAllocator::new(memory_allocator, SubbufferAllocatorCreateInfo::default());
		let mut fps_counter = FpsCounter::<20>::new();
		
		loop {
			let frame = match self.capture() {
				Ok(frame) => frame,
				Err(err) if err.is::<CameraCaptureTimeout>() => continue,
				Err(err) => return Err(err),
			};
			
			fps_counter.tick();
			debug::set_flag("CAMERA_FPS", fps_counter.fps());
			
			let sub_buffer = allocator.from_iter(frame.0.array_chunks::<CHUNK_SIZE>().copied())?;
			
			let mut builder  = AutoCommandBufferBuilder::primary(&*command_buffer_allocator, queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit)?;
			builder.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(sub_buffer, target.clone()))?;
			let command_buffer = builder.build()?;
			
			sender.send((command_buffer, frame.1)).or(Err(CaptureLoopQuitting))?;
		}
	}
}

#[derive(Debug, Error)]
#[error("Timeout while waiting for a frame")]
pub struct CameraCaptureTimeout;

#[derive(Debug, Error)]
#[error("Quitting")]
pub struct CaptureLoopQuitting;
