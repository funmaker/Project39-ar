use std::thread;
use std::sync::{Arc, mpsc};
use anyhow::Result;
use thiserror::Error;
use vulkano::{memory, command_buffer};
use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::buffer::BufferUsage;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, CommandBufferUsage, CopyBufferToImageInfo};
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::device::Queue;
use vulkano::format::Format;
use vulkano::image::{Image, ImageCreateInfo, ImageUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};

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


/// Width of the camera frames.
pub const CAPTURE_WIDTH: u32 = 1920;
/// Height of the camera frames.
pub const CAPTURE_HEIGHT: u32 = 960;
/// Target capture frame rate.
pub const CAPTURE_FPS: u64 = 140;
/// Size of the chunks a frame is copied in to the upload buffer.
pub const CHUNK_SIZE: usize = CAPTURE_WIDTH as usize;

/// Source of passthrough camera frames. Frames are captured on a separate thread.
pub trait Camera: Send + 'static {
	/// Blocks until a new frame is available and returns its `CAPTURE_WIDTH`x`CAPTURE_HEIGHT` BGRA
	/// pixel data along with the head pose at the time of capture, if known.
	///
	/// Returns `CameraCaptureTimeout` if no frame is available yet.
	fn capture(&mut self) -> Result<(&[u8], Option<Isometry3>)>;
	
	/// Spawns the capture thread. Returns the image that frames are uploaded to and a channel
	/// receiving the upload command buffers (with the capture pose) that need to be executed.
	fn start(mut self: Box<Self>, queue: Arc<Queue>, memory_allocator: Arc<StandardMemoryAllocator>, command_buffer_allocator: Arc<StandardCommandBufferAllocator>)
		     -> Result<(Arc<Image>, mpsc::Receiver<(Arc<PrimaryAutoCommandBuffer>, Option<Isometry3>)>)> {
		let target = Image::new(memory_allocator.clone(),
		                        ImageCreateInfo {
			                        extent: [CAPTURE_WIDTH, CAPTURE_HEIGHT, 1],
			                        format: Format::B8G8R8A8_SRGB,
			                        usage: ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
			                        ..ImageCreateInfo::default()
		                        },
		                        AllocationCreateInfo::default())?;
		let ret = target.clone();
		
		let (sender, receiver) = mpsc::sync_channel(1);
		
		thread::Builder::new()
			.name("Camera Capture".to_string())
			.spawn(move || {
				match self.capture_loop(queue, memory_allocator, command_buffer_allocator, target, sender) {
					Ok(()) => {},
					Err(err) if err.is::<CaptureLoopQuitting>() => return,
					Err(err) => panic!("Error while capturing background: {:?}", err),
				}
			})?;
		
		Ok((ret, receiver))
	}
	
	/// Body of the capture thread: captures frames and sends their upload command buffers
	/// until the receiver is dropped.
	fn capture_loop(&mut self, queue: Arc<Queue>, memory_allocator: Arc<StandardMemoryAllocator>, command_buffer_allocator: Arc<StandardCommandBufferAllocator>, target: Arc<Image>, sender: mpsc::SyncSender<(Arc<PrimaryAutoCommandBuffer>, Option<Isometry3>)>) -> Result<()> {
		let allocator = SubbufferAllocator::new(memory_allocator, SubbufferAllocatorCreateInfo {
			buffer_usage: BufferUsage::TRANSFER_SRC,
			memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
			..SubbufferAllocatorCreateInfo::default()
		});
		let mut fps_counter = FpsCounter::<20>::new();
		
		loop {
			let frame = match self.capture() {
				Ok(frame) => frame,
				Err(err) if err.is::<CameraCaptureTimeout>() => continue,
				Err(err) => return Err(err),
			};
			
			fps_counter.tick();
			debug::set_flag("CAMERA_FPS", fps_counter.fps());
			
			let sub_buffer = allocator.from_iter(frame.0.as_chunks::<CHUNK_SIZE>().0.iter().copied())?;
			
			let mut builder  = AutoCommandBufferBuilder::primary(command_buffer_allocator.clone(), queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit)?;
			builder.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(sub_buffer, target.clone()))?;
			let command_buffer = builder.build()?;
			
			sender.send((command_buffer, frame.1)).or(Err(CaptureLoopQuitting))?;
		}
	}
}

/// No camera frame was available in time.
#[derive(Debug, Error)]
#[error("Timeout while waiting for a frame")]
pub struct CameraCaptureTimeout;

/// The receiving end was dropped and the capture thread should exit.
#[derive(Debug, Error)]
#[error("Quitting")]
pub struct CaptureLoopQuitting;
