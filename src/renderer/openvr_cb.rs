use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use vulkano::command_buffer::pool::{CommandPool, CommandPoolBuilderAlloc};
use vulkano::command_buffer::sys::{UnsafeCommandBuffer, UnsafeCommandBufferBuilder, UnsafeCommandBufferBuilderPipelineBarrier};
use vulkano::command_buffer::{CommandBufferLevel, CommandBufferUsage, PrimaryCommandBuffer, CommandBufferExecError};
use vulkano::device::{Device, DeviceOwned, Queue};
use vulkano::instance::QueueFamily;
use vulkano::image::{AttachmentImage, ImageAccess, ImageLayout};
use vulkano::sync::{PipelineStages, AccessFlags, AccessCheckError, GpuFuture, AccessError};
use vulkano::command_buffer::synced::{SyncCommandBuffer, SyncCommandBufferBuilder};
use vulkano::buffer::BufferAccess;
use vulkano::command_buffer::pool::standard::StandardCommandPoolAlloc;
use vulkano::OomError;


pub struct OpenVRCommandBuffer<P = StandardCommandPoolAlloc> {
	inner: SyncCommandBuffer,
	#[allow(dead_code)] pool_alloc: P, // Safety: must be dropped after `inner`
	in_use: AtomicBool,
	image: Arc<AttachmentImage>,
}

impl OpenVRCommandBuffer<StandardCommandPoolAlloc> {
	pub unsafe fn start(image: Arc<AttachmentImage>, device: Arc<Device>, queue_family: QueueFamily) -> Result<OpenVRCommandBuffer, OomError> {
		Self::new(image, None, Some(ImageLayout::TransferSrcOptimal), device, queue_family)
	}
	
	pub unsafe fn end(image: Arc<AttachmentImage>, device: Arc<Device>, queue_family: QueueFamily) -> Result<OpenVRCommandBuffer, OomError> {
		Self::new(image, Some(ImageLayout::TransferSrcOptimal), None, device, queue_family)
	}
	
	unsafe fn new(image: Arc<AttachmentImage>, from_layout: Option<ImageLayout>, to_layout: Option<ImageLayout>, device: Arc<Device>, queue_family: QueueFamily) -> Result<OpenVRCommandBuffer, OomError> {
		let pool = Device::standard_command_pool(&device, queue_family);
		let pool_builder_alloc = pool.alloc(false, 1)?
			.next()
			.expect("Requested one command buffer from the command pool, but got zero.");
		
		let mut builder = UnsafeCommandBufferBuilder::new(pool_builder_alloc.inner(), CommandBufferLevel::primary(), CommandBufferUsage::MultipleSubmit)?;
		let mut barrier = UnsafeCommandBufferBuilderPipelineBarrier::new();
		
		barrier.add_image_memory_barrier(
			&image,
			image.current_miplevels_access(),
			image.current_layer_levels_access(),
			PipelineStages {
				bottom_of_pipe: from_layout.is_none(),
				transfer: from_layout.is_some(),
				..PipelineStages::none()
			},
			AccessFlags {
				transfer_read: from_layout.is_some(),
				..AccessFlags::none()
			},
			PipelineStages {
				top_of_pipe: to_layout.is_none(),
				transfer: to_layout.is_some(),
				..PipelineStages::none()
			},
			AccessFlags {
				transfer_read: to_layout.is_some(),
				..AccessFlags::none()
			},
			false,
			None,
			from_layout.unwrap_or(image.final_layout_requirement()),
			to_layout.unwrap_or(image.final_layout_requirement()),
		);
		
		builder.pipeline_barrier(&barrier);
		
		let sync = SyncCommandBufferBuilder::from_unsafe_cmd(builder, false, false).build()?;
		
		Ok(OpenVRCommandBuffer {
			inner: sync,
			pool_alloc: pool_builder_alloc.into_alloc(),
			in_use: AtomicBool::new(false),
			image,
		})
	}
}

unsafe impl<P> DeviceOwned for OpenVRCommandBuffer<P> {
	fn device(&self) -> &Arc<Device> {
		self.inner.device()
	}
}

unsafe impl<P> PrimaryCommandBuffer for OpenVRCommandBuffer<P> {
	fn inner(&self) -> &UnsafeCommandBuffer {
		self.inner.as_ref()
	}
	
	fn lock_submit(&self, future: &dyn GpuFuture, queue: &Queue) -> Result<(), CommandBufferExecError> {
		let already_in_use = self.in_use.swap(true, Ordering::SeqCst);
		if already_in_use {
			return Err(CommandBufferExecError::ExclusiveAlreadyInUse);
		}
		
		let err = match self.inner.lock_submit(future, queue) {
			Ok(()) => return Ok(()),
			Err(err) => err,
		};
		
		// If `self.inner.lock_submit()` failed, we revert action.
		self.in_use.store(false, Ordering::SeqCst);
		
		Err(err)
	}
	
	unsafe fn unlock(&self) {
		// Because of panic safety, we unlock the inner command buffer first.
		self.inner.unlock();
		
		let old_val = self.in_use.swap(false, Ordering::SeqCst);
		debug_assert!(old_val);
	}
	
	fn check_buffer_access(&self, buffer: &dyn BufferAccess, _exclusive: bool, _queue: &Queue) -> Result<Option<(PipelineStages, AccessFlags)>, AccessCheckError> {
		if buffer.conflicts_image(&self.image) {
			Err(AccessCheckError::Denied(AccessError::AlreadyInUse))
		} else {
			Err(AccessCheckError::Unknown)
		}
	}
	
	fn check_image_access(&self, image: &dyn ImageAccess, _layout: ImageLayout, _exclusive: bool, _queue: &Queue) -> Result<Option<(PipelineStages, AccessFlags)>, AccessCheckError> {
		if image.conflicts_image(&self.image) {
			Err(AccessCheckError::Denied(AccessError::AlreadyInUse))
		} else {
			Err(AccessCheckError::Unknown)
		}
	}
}


