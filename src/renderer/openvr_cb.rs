use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use vulkano::command_buffer::pool::{CommandPool, CommandPoolBuilderAlloc};
use vulkano::command_buffer::sys::{UnsafeCommandBuffer, UnsafeCommandBufferBuilder, UnsafeCommandBufferBuilderPipelineBarrier};
use vulkano::command_buffer::{CommandBufferLevel, CommandBufferUsage, PrimaryCommandBuffer, CommandBufferExecError};
use vulkano::device::{Device, DeviceOwned, Queue};
use vulkano::device::physical::QueueFamily;
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
	image_initial_layout: ImageLayout,
	image_final_layout: ImageLayout,
	image_final_stages: PipelineStages,
	image_final_access: AccessFlags,
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
		
		let current_layout = from_layout.unwrap_or(image.final_layout_requirement());
		let source_stage = PipelineStages {
			bottom_of_pipe: from_layout.is_none(),
			transfer: from_layout.is_some(),
			..PipelineStages::none()
		};
		let source_access = AccessFlags {
			transfer_read: from_layout.is_some(),
			..AccessFlags::none()
		};
		
		let new_layout = to_layout.unwrap_or(image.final_layout_requirement());
		let destination_stage = PipelineStages {
			top_of_pipe: to_layout.is_none(),
			transfer: to_layout.is_some(),
			..PipelineStages::none()
		};
		let destination_access = AccessFlags {
			transfer_read: to_layout.is_some(),
			..AccessFlags::none()
		};
		
		barrier.add_image_memory_barrier(
			&image,
			image.current_miplevels_access(),
			image.current_layer_levels_access(),
			source_stage,
			source_access,
			destination_stage,
			destination_access,
			false,
			None,
			current_layout,
			new_layout,
		);
		
		builder.pipeline_barrier(&barrier);
		
		let sync = SyncCommandBufferBuilder::from_unsafe_cmd(builder, false, false).build()?;
		
		Ok(OpenVRCommandBuffer {
			inner: sync,
			pool_alloc: pool_builder_alloc.into_alloc(),
			in_use: AtomicBool::new(false),
			image,
			image_initial_layout: current_layout,
			image_final_layout: new_layout,
			image_final_stages: destination_stage,
			image_final_access: destination_access,
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
		
		// Only lock when image leaves its preferred layout buffer
		if self.image_initial_layout != self.image.final_layout_requirement() {
			return Ok(());
		}
		
		let prev_err = match future.check_image_access(
			&self.image,
			self.image_initial_layout,
			true,
			queue,
		) {
			Ok(_) => {
				unsafe {
					self.image.increase_gpu_lock();
				}
				return Ok(());
			}
			Err(err) => err,
		};
		
		match (
			self.image.try_gpu_lock(
				true,
				false,
				self.image_initial_layout,
			),
			prev_err,
		) {
			(Ok(_), _) => return Ok(()),
			(Err(err), AccessCheckError::Unknown)
			| (_, AccessCheckError::Denied(err)) => {
				
				// Lock failed, we revert action.
				self.in_use.store(false, Ordering::SeqCst);
				
				Err(CommandBufferExecError::AccessError {
					error: err,
					command_name: "OpenVRBarrier".into(),
					command_param: "Image".into(),
					command_offset: 0,
				})
			}
		}
	}
	
	unsafe fn unlock(&self) {
		// Only unlock on when image reverts to its preferred layout buffer
		if self.image_final_layout == self.image.final_layout_requirement() {
			self.image.unlock(None);
		}
		
		let old_val = self.in_use.swap(false, Ordering::SeqCst);
		debug_assert!(old_val);
	}
	
	fn check_buffer_access(&self, _buffer: &dyn BufferAccess, _exclusive: bool, _queue: &Queue) -> Result<Option<(PipelineStages, AccessFlags)>, AccessCheckError> {
		Err(AccessCheckError::Unknown)
	}
	
	fn check_image_access(&self, image: &dyn ImageAccess, layout: ImageLayout, _exclusive: bool, _queue: &Queue) -> Result<Option<(PipelineStages, AccessFlags)>, AccessCheckError> {
		// TODO: check the queue family
		if self.image.conflict_key() == image.conflict_key() {
			if layout != ImageLayout::Undefined && self.image_final_layout != layout {
				return Err(AccessCheckError::Denied(
					AccessError::UnexpectedImageLayout {
						allowed: self.image_final_layout,
						requested: layout,
					},
				));
			}
			
			return Ok(Some((self.image_final_stages, self.image_final_access)));
		} else {
			Err(AccessCheckError::Unknown)
		}
	}
}


