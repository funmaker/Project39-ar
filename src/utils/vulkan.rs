use std::ffi::CString;
use std::sync::Arc;
use err_derive::Error;
use openvr::{VkInstance_T, VkPhysicalDevice_T, Compositor, VkDevice_T, VkQueue_T};
use vulkano::{buffer, VulkanObject, Handle, command_buffer, memory};
use vulkano::buffer::{BufferContents, BufferCreateInfo, Subbuffer, Buffer, BufferUsage};
use vulkano::buffer::allocator::SubbufferAllocator;
use vulkano::command_buffer::{AutoCommandBufferBuilder, CopyBufferInfo, PrimaryAutoCommandBuffer};
use vulkano::device::{Device, Queue};
use vulkano::device::physical::PhysicalDevice;
use vulkano::format::ClearValue;
use vulkano::image::{AttachmentImage, ImageAccess, StorageImage, ImmutableImage};
use vulkano::instance::Instance;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocator, MemoryUsage};
use vulkano::render_pass::Framebuffer;


pub fn vulkan_device_extensions_required(compositor: &Compositor, physical: &PhysicalDevice) -> Vec<CString> {
	unsafe { compositor.vulkan_device_extensions_required(physical.as_ptr()) }
}

#[derive(Clone, Debug)]
pub struct FramebufferBundle {
	pub framebuffer: Arc<Framebuffer>,
	pub main_image: Arc<AttachmentImage>,
	pub ssaa: f32,
	pub clear_values: Vec<Option<ClearValue>>,
}

impl FramebufferBundle {
	pub fn size(&self) -> (u32, u32) {
		let extent = self.framebuffer.extent();
		(extent[0], extent[1])
	}
}

pub trait SubbufferAllocatorEx {
	fn from_iter<T: Sized + BufferContents>(&self, iter: impl ExactSizeIterator<Item = T>) -> Result<Subbuffer<[T]>, SubbufferAllocatorExError>;
}

impl SubbufferAllocatorEx for SubbufferAllocator {
	fn from_iter<T: Sized + BufferContents>(&self, iter: impl ExactSizeIterator<Item = T>) -> Result<Subbuffer<[T]>, SubbufferAllocatorExError> {
		let subbuffer = self.allocate_slice(iter.len() as u64)?;
		
		{
			let mut buf = subbuffer.write()?;
			
			for (pos, item) in iter.enumerate() {
				buf[pos] = item;
			}
		}
		
		Ok(subbuffer)
	}
}

#[derive(Debug, Error)]
pub enum SubbufferAllocatorExError {
	#[error(display = "{}", _0)] AllocationCreationError(#[error(source)] memory::allocator::AllocationCreationError),
	#[error(display = "{}", _0)] BufferError(#[error(source)] buffer::BufferError),
}

pub trait IntoInfo<T>: Sized {
	#[must_use]
	fn into_info(self) -> T;
}

impl IntoInfo<BufferCreateInfo> for BufferUsage {
	fn into_info(self) -> BufferCreateInfo {
		BufferCreateInfo {
			usage: self,
			..BufferCreateInfo::default()
		}
	}
}

impl IntoInfo<AllocationCreateInfo> for MemoryUsage {
	fn into_info(self) -> AllocationCreateInfo {
		AllocationCreateInfo {
			usage: self,
			..AllocationCreateInfo::default()
		}
	}
}

pub trait BufferEx {
	fn upload_data<T>(allocator: &(impl MemoryAllocator + ?Sized),
	                  buffer_info: BufferCreateInfo,
	                  data: T,
	                  cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>)
	                  -> Result<Subbuffer<T>, UploadError>
	                  where T: BufferContents;
	
	fn upload_iter<T, I>(allocator: &(impl MemoryAllocator + ?Sized),
	                     buffer_info: BufferCreateInfo,
	                     iter: I,
	                     cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>)
	                     -> Result<Subbuffer<[T]>, UploadError>
	                     where T: BufferContents,
	                           I: IntoIterator<Item = T>,
	                           I::IntoIter: ExactSizeIterator;
}

impl BufferEx for Buffer {
	fn upload_data<T>(allocator: &(impl MemoryAllocator + ?Sized),
	                  buffer_info: BufferCreateInfo,
	                  data: T,
	                  cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>)
	                  -> Result<Subbuffer<T>, UploadError>
	                  where T: BufferContents {
		let device_buffer = Buffer::new_sized(allocator,
		                                      BufferCreateInfo {
			                                      usage: buffer_info.usage | BufferUsage::TRANSFER_DST,
			                                      ..buffer_info
		                                      },
		                                      MemoryUsage::DeviceOnly.into_info())?;
		
		let upload_buffer = Buffer::from_data(allocator,
		                                      BufferUsage::TRANSFER_SRC.into_info(),
		                                      MemoryUsage::Upload.into_info(),
		                                      data)?;
		
		cbb.copy_buffer(CopyBufferInfo::buffers(upload_buffer, device_buffer.clone()))?;
		
		Ok(device_buffer)
	}
	
	fn upload_iter<T, I>(allocator: &(impl MemoryAllocator + ?Sized),
	                     buffer_info: BufferCreateInfo,
	                     iter: I,
	                     cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>)
	                     -> Result<Subbuffer<[T]>, UploadError>
	                     where T: BufferContents,
	                           I: IntoIterator<Item=T>,
	                           I::IntoIter: ExactSizeIterator {
		let iter = iter.into_iter();
		
		let device_buffer = Buffer::new_slice(allocator,
		                                      BufferCreateInfo {
			                                      usage: buffer_info.usage | BufferUsage::TRANSFER_DST,
			                                      ..buffer_info
		                                      },
		                                      MemoryUsage::DeviceOnly.into_info(),
		                                      iter.len() as u64)?;
		
		let upload_buffer = Buffer::from_iter(allocator,
		                                      BufferUsage::TRANSFER_SRC.into_info(),
		                                      MemoryUsage::Upload.into_info(),
		                                      iter)?;
		
		cbb.copy_buffer(CopyBufferInfo::buffers(upload_buffer, device_buffer.clone()))?;
		
		Ok(device_buffer)
	}
}

#[derive(Debug, Error)]
pub enum UploadError {
	#[error(display = "{}", _0)] BufferError(#[error(source)] buffer::BufferError),
	#[error(display = "{}", _0)] CopyError(#[error(source)] command_buffer::CopyError),
}

pub trait OpenVRPtr {
	type PtrType;
	
	fn as_ptr(&self) -> Self::PtrType;
}

impl OpenVRPtr for Instance {
	type PtrType = *mut VkInstance_T;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.handle().as_raw() as Self::PtrType
	}
}

impl OpenVRPtr for PhysicalDevice {
	type PtrType = *mut VkPhysicalDevice_T;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.handle().as_raw() as Self::PtrType
	}
}

impl OpenVRPtr for Device {
	type PtrType = *mut VkDevice_T;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.handle().as_raw() as Self::PtrType
	}
}

impl OpenVRPtr for Queue {
	type PtrType = *mut VkQueue_T;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.handle().as_raw() as Self::PtrType
	}
}

impl OpenVRPtr for AttachmentImage {
	type PtrType = u64;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.inner().image.handle().as_raw()
	}
}

impl OpenVRPtr for ImmutableImage {
	type PtrType = u64;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.inner().image.handle().as_raw()
	}
}

impl OpenVRPtr for StorageImage {
	type PtrType = u64;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.inner().image.handle().as_raw()
	}
}

