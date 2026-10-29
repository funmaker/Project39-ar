use std::ffi::CString;
use std::sync::Arc;
use anyhow::Result;
use openvr::{VkInstance_T, VkPhysicalDevice_T, Compositor, VkDevice_T, VkQueue_T};
use smallvec::SmallVec;
use vulkano::{Handle, VulkanObject};
use vulkano::buffer::{BufferContents, BufferCreateInfo, Subbuffer, Buffer, BufferUsage};
use vulkano::buffer::allocator::SubbufferAllocator;
use vulkano::command_buffer::{AutoCommandBufferBuilder, CopyBufferInfo, PrimaryAutoCommandBuffer};
use vulkano::command_buffer::{BlitImageInfo, BufferImageCopy, CopyBufferToImageInfo, ImageBlit};
use vulkano::device::{Device, Queue};
use vulkano::device::physical::PhysicalDevice;
use vulkano::format::ClearValue;
use vulkano::image::{Image, ImageCreateInfo, ImageSubresourceLayers, ImageUsage};
use vulkano::image::mip_level_extent;
use vulkano::image::sampler::Filter;
use vulkano::instance::Instance;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};
use vulkano::render_pass::Framebuffer;
use vulkano::sync::Sharing;

pub fn vulkan_device_extensions_required(compositor: &Compositor, physical: &PhysicalDevice) -> Vec<CString> {
	unsafe { compositor.vulkan_device_extensions_required(physical.as_ptr()) }
}

#[derive(Clone, Debug)]
pub struct FramebufferBundle {
	pub framebuffer: Arc<Framebuffer>,
	pub main_image: Arc<Image>,
	pub ssaa: f32,
	pub clear_values: Vec<Option<ClearValue>>,
}

impl FramebufferBundle {
	pub fn extent(&self) -> (u32, u32) {
		let extent = self.framebuffer.extent();
		(extent[0], extent[1])
	}
}

pub trait SubbufferAllocatorEx {
	fn from_iter<T: Sized + BufferContents>(&self, iter: impl ExactSizeIterator<Item = T>) -> Result<Subbuffer<[T]>>;
}

impl SubbufferAllocatorEx for SubbufferAllocator {
	fn from_iter<T: Sized + BufferContents>(&self, iter: impl ExactSizeIterator<Item = T>) -> Result<Subbuffer<[T]>> {
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

impl IntoInfo<AllocationCreateInfo> for MemoryTypeFilter {
	fn into_info(self) -> AllocationCreateInfo {
		AllocationCreateInfo {
			memory_type_filter: self,
			..AllocationCreateInfo::default()
		}
	}
}

pub trait BufferEx {
	fn upload_data<T>(allocator: &Arc<StandardMemoryAllocator>,
	                  buffer_info: BufferCreateInfo,
	                  data: T,
	                  cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>)
	                  -> Result<Subbuffer<T>>
	                  where T: BufferContents;
	
	fn upload_iter<T, I>(allocator: &Arc<StandardMemoryAllocator>,
	                     buffer_info: BufferCreateInfo,
	                     iter: I,
	                     cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>)
	                     -> Result<Subbuffer<[T]>>
	                     where T: BufferContents,
	                           I: IntoIterator<Item = T>,
	                           I::IntoIter: ExactSizeIterator;
}

impl BufferEx for Buffer {
	fn upload_data<T>(allocator: &Arc<StandardMemoryAllocator>,
	                  buffer_info: BufferCreateInfo,
	                  data: T,
	                  cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>)
	                  -> Result<Subbuffer<T>>
	                  where T: BufferContents {
		let device_buffer = Buffer::new_sized(allocator.clone(),
		                                      BufferCreateInfo {
			                                      usage: buffer_info.usage | BufferUsage::TRANSFER_DST,
			                                      ..buffer_info
		                                      },
		                                      MemoryTypeFilter::PREFER_DEVICE.into_info())?;
		
		let upload_buffer = Buffer::from_data(allocator.clone(),
		                                      BufferUsage::TRANSFER_SRC.into_info(),
		                                      (MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE).into_info(),
		                                      data)?;
		
		cbb.copy_buffer(CopyBufferInfo::buffers(upload_buffer, device_buffer.clone()))?;
		
		Ok(device_buffer)
	}
	
	fn upload_iter<T, I>(allocator: &Arc<StandardMemoryAllocator>,
	                     buffer_info: BufferCreateInfo,
	                     iter: I,
	                     cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>)
	                     -> Result<Subbuffer<[T]>>
	                     where T: BufferContents,
	                           I: IntoIterator<Item=T>,
	                           I::IntoIter: ExactSizeIterator {
		let iter = iter.into_iter();
		
		let device_buffer = Buffer::new_slice(allocator.clone(),
		                                      BufferCreateInfo {
			                                      usage: buffer_info.usage | BufferUsage::TRANSFER_DST,
			                                      ..buffer_info
		                                      },
		                                      MemoryTypeFilter::PREFER_DEVICE.into_info(),
		                                      iter.len() as u64)?;
		
		let upload_buffer = Buffer::from_iter(allocator.clone(),
		                                      BufferUsage::TRANSFER_SRC.into_info(),
		                                      (MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE).into_info(),
		                                      iter)?;
		
		cbb.copy_buffer(CopyBufferInfo::buffers(upload_buffer, device_buffer.clone()))?;
		
		Ok(device_buffer)
	}
}


pub trait ImageEx {
	fn upload_iter(allocator: &Arc<StandardMemoryAllocator>,
	               iter: impl ExactSizeIterator<Item = u8>,
	               create_info: ImageCreateInfo,
	               allocation_info: AllocationCreateInfo,
	               cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>)
	               -> Result<Arc<Self>>;
	
	fn generate_mipmaps(self: &Arc<Self>, cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<()>;
}

impl ImageEx for Image {
	fn upload_iter(allocator: &Arc<StandardMemoryAllocator>,
	               iter: impl ExactSizeIterator<Item=u8>,
	               mut create_info: ImageCreateInfo,
	               allocation_info: AllocationCreateInfo,
	               cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>)
	               -> Result<Arc<Self>> {
		let need_mipmaps = create_info.mip_levels > 1;
		
		create_info.usage |= ImageUsage::TRANSFER_DST;
		if need_mipmaps {
			create_info.usage |= ImageUsage::TRANSFER_SRC;
		}
		
		let source = Buffer::from_iter(allocator.clone(),
		                               BufferCreateInfo {
			                               usage: BufferUsage::TRANSFER_SRC,
			                               ..BufferCreateInfo::default()
		                               },
		                               AllocationCreateInfo {
			                               memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
			                               ..AllocationCreateInfo::default()
		                               },
		                               iter)?;
		
		let main_region = BufferImageCopy {
			image_subresource: ImageSubresourceLayers::from_parameters(create_info.format, create_info.array_layers),
			image_extent: create_info.extent,
			..BufferImageCopy::default()
		};
		
		let image = Image::new(allocator.clone(), create_info, allocation_info)?;
		
		cbb.copy_buffer_to_image(CopyBufferToImageInfo {
			regions: smallvec::smallvec![main_region],
			..CopyBufferToImageInfo::buffer_image(source, image.clone())
		})?;
		
		if need_mipmaps {
			image.generate_mipmaps(cbb)?;
		}
		
		Ok(image)
	}
	
	fn generate_mipmaps(self: &Arc<Self>, cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<()> {
		let extent = self.extent();
		
		for level in 1..self.mip_levels() {
			let src_size = mip_level_extent(extent, level - 1).unwrap();
			let dst_size = mip_level_extent(extent, level).unwrap();
			
			cbb.blit_image(BlitImageInfo {
				regions: smallvec::smallvec![ImageBlit {
					src_subresource: ImageSubresourceLayers {
						mip_level: level - 1,
						..self.subresource_layers()
					},
					src_offsets: [[0; 3], src_size],
					dst_subresource: ImageSubresourceLayers {
						mip_level: level,
						..self.subresource_layers()
					},
					dst_offsets: [[0; 3], dst_size],
					..ImageBlit::default()
				}],
				filter: Filter::Linear,
				..BlitImageInfo::images(self.clone(), self.clone())
			})?;
		}
		
		Ok(())
	}
}

pub trait SharingEx<I> {
	fn from_iters(indices: impl IntoIterator<IntoIter=I>) -> Self;
}

impl<'a, I> SharingEx<I> for Sharing<SmallVec<[u32; 4]>>
where I: ExactSizeIterator<Item=&'a u32> {
	fn from_iters(indices: impl IntoIterator<IntoIter=I>) -> Self {
		let indices = indices.into_iter();
		if indices.len() >= 2 {
			Sharing::Concurrent(SmallVec::from_iter(indices.copied()))
		} else {
			Sharing::Exclusive
		}
	}
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

impl OpenVRPtr for Image {
	type PtrType = u64;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.handle().as_raw()
	}
}
