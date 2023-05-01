use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::{Mutex, MutexGuard};
use vulkano::{DeviceSize, OomError, SafeDeref, VulkanObject};
use vulkano::command_buffer::allocator::{CommandBufferAlloc, CommandBufferAllocator, StandardCommandBufferAlloc, StandardCommandBufferAllocator};
use vulkano::command_buffer::pool::{CommandPool, CommandPoolAlloc};
use vulkano::command_buffer::sys::{CommandBufferBeginInfo, UnsafeCommandBuffer, UnsafeCommandBufferBuilder};
use vulkano::command_buffer::{CommandBufferLevel, CommandBufferUsage, CommandBufferExecError, PrimaryCommandBufferAbstract, CommandBufferExecFuture, CommandBufferState, CommandBufferResourcesUsage};
use vulkano::device::{Device, DeviceOwned, Queue};
use vulkano::image::{sys::Image, AttachmentImage, ImageLayout, ImageAccess};
use vulkano::sync::{DependencyInfo, ImageMemoryBarrier};
use vulkano::sync::future::{GpuFuture, NowFuture};
use vulkano::sync::PipelineStages;
use vulkano::command_buffer::synced::{SyncCommandBuffer, SyncCommandBufferBuilder};
use vulkano::command_buffer::allocator::CommandBufferBuilderAlloc;
use vulkano::sync::AccessFlags;


pub struct OpenVRCommandBuffer<A = StandardCommandBufferAlloc> {
	inner: SyncCommandBuffer,
	#[allow(dead_code)] _alloc: A, // Safety: must be dropped after `inner`
	// image: Arc<AttachmentImage>,
	// image_final_layout: ImageLayout,
	// image_final_stages: PipelineStages,
	// image_final_access: AccessFlags,
	
	state: Mutex<CommandBufferState>,
}

impl OpenVRCommandBuffer {
	pub unsafe fn start(allocator: &StandardCommandBufferAllocator, image: Arc<impl ImageAccess>, queue_family_index: u32) -> Result<OpenVRCommandBuffer, OomError> {
		Self::new(allocator, image, None, Some(ImageLayout::TransferSrcOptimal), queue_family_index)
	}
	
	pub unsafe fn end(allocator: &StandardCommandBufferAllocator, image: Arc<impl ImageAccess>, queue_family_index: u32) -> Result<OpenVRCommandBuffer, OomError> {
		Self::new(allocator, image, Some(ImageLayout::TransferSrcOptimal), None, queue_family_index)
	}
	
	unsafe fn new(allocator: &StandardCommandBufferAllocator, image: Arc<impl ImageAccess>, from_layout: Option<ImageLayout>, to_layout: Option<ImageLayout>, queue_family_index: u32) -> Result<OpenVRCommandBuffer, OomError> {
		let builder_alloc = allocator.allocate(queue_family_index, CommandBufferLevel::Primary, 1)?
		                             .next()
		                             .expect("Requested one command buffer from the command pool, but got zero.");
		
		let mut builder = UnsafeCommandBufferBuilder::new(builder_alloc.inner(), CommandBufferBeginInfo {
			usage: CommandBufferUsage::MultipleSubmit,
			..CommandBufferBeginInfo::default()
		})?;
		
		let mut dependency_info = DependencyInfo::default();
		
		let (src_stages, src_access) = if from_layout.is_some() {
			(PipelineStages::ALL_TRANSFER, AccessFlags::TRANSFER_READ)
		} else {
			(PipelineStages::BOTTOM_OF_PIPE, AccessFlags::empty())
		};
		
		let (dst_stages, dst_access) = if to_layout.is_some() {
			(PipelineStages::ALL_TRANSFER, AccessFlags::TRANSFER_READ)
		} else {
			(PipelineStages::TOP_OF_PIPE, AccessFlags::empty())
		};
		
		let old_layout = from_layout.unwrap_or(image.final_layout_requirement());
		let new_layout = to_layout.unwrap_or(image.final_layout_requirement());
		
		dependency_info.image_memory_barriers.push(ImageMemoryBarrier {
			src_stages,
			src_access,
			dst_stages,
			dst_access,
			old_layout,
			new_layout,
			subresource_range: image.subresource_range(),
			..ImageMemoryBarrier::image(image.inner().image.clone())
		});
		
		builder.pipeline_barrier(&dependency_info);
		
		let sync = SyncCommandBufferBuilder::from_unsafe_cmd(builder, CommandBufferLevel::Primary, false).build()?;
		
		Ok(OpenVRCommandBuffer {
			inner: sync,
			_alloc: builder_alloc.into_alloc(),
			// image,
			// image_final_layout: new_layout,
			// image_final_stages: dst_stages,
			// image_final_access: dst_access,
			
			state: Mutex::new(Default::default()),
		})
	}
}

unsafe impl<A> VulkanObject for OpenVRCommandBuffer<A> {
	type Handle = ash::vk::CommandBuffer;
	
	fn handle(&self) -> Self::Handle {
		self.inner.as_ref().handle()
	}
}

unsafe impl<A> DeviceOwned for OpenVRCommandBuffer<A> {
	fn device(&self) -> &Arc<Device> {
		self.inner.device()
	}
}

unsafe impl<A> PrimaryCommandBufferAbstract for OpenVRCommandBuffer<A>
where A: CommandBufferAlloc {
	fn usage(&self) -> CommandBufferUsage {
		CommandBufferUsage::MultipleSubmit
	}
	
	fn state(&self) -> MutexGuard<'_, CommandBufferState> {
		self.state.lock().into()
	}
	
	fn resources_usage(&self) -> &CommandBufferResourcesUsage {
		self.inner.resources_usage()
	}
}


