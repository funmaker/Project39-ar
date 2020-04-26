use std::thread;
use std::sync::Arc;
use std::time::Instant;
use std::sync::mpsc;
use std::sync::mpsc::SendError;
use std::convert::TryInto;

use err_derive::Error;
use vulkano::buffer::{CpuBufferPool, BufferView, BufferSlice, BufferAccess};
use vulkano::device::Queue;
use vulkano::image::{AttachmentImage, ImageCreationError, ImageUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CopyBufferImageError, BuildError, CommandBufferExecError, AutoCommandBuffer};
use vulkano::{format, OomError};
use vulkano::format::{AcceptsPixels, IncompatiblePixelsType};
use vulkano::buffer::cpu_pool::CpuBufferPoolChunk;
use vulkano::buffer::view::BufferViewCreationError;
use vulkano::memory::DeviceMemoryAllocError;

pub const CAPTURE_INDEX: usize = 0;
pub const CAPTURE_WIDTH: u32 = 1920;
pub const CAPTURE_HEIGHT: u32 = 960;
pub const CAPTURE_FPS: u64 = 60;
pub const BUFFER_SIZE: usize = (CAPTURE_WIDTH * CAPTURE_HEIGHT * 4) as usize;

pub struct Camera {
	inner: escapi::Device,
	buffer: CpuBufferPool<[u8; 32]>,
	last_capture: Instant,
}

impl Camera {
	pub fn new(device: &Arc<vulkano::device::Device>) -> Result<Camera, CameraError> {
		let inner = escapi::init(CAPTURE_INDEX, CAPTURE_WIDTH, CAPTURE_HEIGHT, CAPTURE_FPS)?;
		let buffer = CpuBufferPool::upload(device.clone());
		
		dprintln!("Camera {}: {}x{}", inner.name(), inner.capture_width(), inner.capture_height());
		
		Ok(Camera{
			inner,
			buffer,
			last_capture: Instant::now(),
		})
	}
	
	pub fn start(mut self, queue: Arc<Queue>) -> Result<(Arc<AttachmentImage<format::B8G8R8A8Unorm>>, mpsc::Receiver<AutoCommandBuffer>), CameraStartError> {
		let target = AttachmentImage::with_usage(queue.device().clone(),
		                                         [CAPTURE_WIDTH, CAPTURE_HEIGHT],
		                                         format::B8G8R8A8Unorm,
		                                         ImageUsage { transfer_source: true,
		                                                      transfer_destination: true,
		                                                      ..ImageUsage::none() })?;
		let ret = target.clone();
		
		let (sender, receiver) = mpsc::channel();
		
		thread::spawn(move || {
			match self.capture(queue, target, sender) {
				Ok(()) => {},
				Err(err) => panic!("Error while capturing camera: {:?}", err),
			}
		});
		
		Ok((ret, receiver))
	}
	
	fn capture(&mut self, queue: Arc<Queue>, target: Arc<AttachmentImage<format::B8G8R8A8Unorm>>, sender: mpsc::Sender<AutoCommandBuffer>) -> Result<(), CaptureError> {
		loop {
			let mut instant = Instant::now();
			let frame = match self.inner.capture() {
				Ok(frame) => frame,
				Err(escapi::Error::CaptureTimeout) => continue,
				Err(err) => return Err(err.into()),
			};
			
			println!("{} FPS\t{}", 1.0 / self.last_capture.elapsed().as_secs_f32(), frame.len());
			self.last_capture = Instant::now();
			
			let sub_buffer = self.buffer.chunk(frame.chunks_exact(32).map(|c| c.try_into().unwrap()))?;
			let sub_slice: BufferSlice<[u8], _> = unsafe { sub_buffer.into_buffer_slice().reinterpret() };
			
			let command_buffer = AutoCommandBufferBuilder::new(queue.device().clone(), queue.family())?
			                                              .copy_buffer_to_image(sub_slice, target.clone())?
			                                              .build()?;
			
			sender.send(command_buffer)?;
		}
	}
}

#[derive(Debug, Error)]
pub enum CameraError {
	#[error(display = "{}", _0)] EscapiError(#[error(source)] escapi::Error),
}

#[derive(Debug, Error)]
pub enum CameraStartError {
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] ImageCreationError),
}

#[derive(Debug, Error)]
pub enum CaptureError {
	#[error(display = "{}", _0)] EscapiError(#[error(source)] escapi::Error),
	#[error(display = "{}", _0)] OomError(#[error(source)] OomError),
	#[error(display = "{}", _0)] CopyBufferImageError(#[error(source)] CopyBufferImageError),
	#[error(display = "{}", _0)] BuildError(#[error(source)] BuildError),
	#[error(display = "{}", _0)] CommandBufferExecError(#[error(source)] CommandBufferExecError),
	#[error(display = "{}", _0)] SendError(#[error(source)] SendError<AutoCommandBuffer>),
	#[error(display = "{}", _0)] BufferViewCreationError(#[error(source)] BufferViewCreationError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] DeviceMemoryAllocError),
}
