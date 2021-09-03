use std::ffi::CString;
use vulkano::instance::Instance;
use vulkano::{VulkanObject, SynchronizedVulkanObject, Handle};
use vulkano::device::{Device, Queue};
use vulkano::device::physical::PhysicalDevice;
use vulkano::image::{AttachmentImage, ImageAccess, StorageImage, ImmutableImage};
use openvr::{VkInstance_T, VkPhysicalDevice_T, Compositor, VkDevice_T, VkQueue_T};
use image::DynamicImage;

mod vec_future;
pub mod from_args;
mod fps_counter;

pub use vec_future::VecFuture;
pub use fps_counter::FpsCounter;


// Images

pub trait ImageEx {
	fn into_pre_mul_iter(self) -> std::vec::IntoIter<u8>;
	fn has_alpha(&self) -> bool;
}

impl ImageEx for DynamicImage {
	fn into_pre_mul_iter(self) -> std::vec::IntoIter<u8> {
		let has_alpha = self.has_alpha();
		let mut data = self.into_rgba8();
		
		if has_alpha {
			for pixel in data.pixels_mut() {
				pixel[0] = (pixel[0] as u16 * pixel[3] as u16 / 255) as u8;
				pixel[1] = (pixel[1] as u16 * pixel[3] as u16 / 255) as u8;
				pixel[2] = (pixel[2] as u16 * pixel[3] as u16 / 255) as u8;
			}
		}
		
		data.into_vec().into_iter()
	}
	
	fn has_alpha(&self) -> bool {
		match self {
			DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgb16(_) | DynamicImage::ImageBgr8(_) | DynamicImage::ImageLuma16(_) => false,
			DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_) | DynamicImage::ImageRgba16(_) | DynamicImage::ImageBgra8(_)| DynamicImage::ImageLumaA16(_) => true,
		}
	}
}

// Vulkan

pub fn vulkan_device_extensions_required(compositor: &Compositor, physical: &PhysicalDevice) -> Vec<CString> {
	unsafe { compositor.vulkan_device_extensions_required(physical.as_ptr()) }
}

pub trait OpenVRPtr {
	type PtrType;
	
	fn as_ptr(&self) -> Self::PtrType;
}

impl OpenVRPtr for Instance {
	type PtrType = *mut VkInstance_T;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.internal_object().as_raw() as Self::PtrType
	}
}

impl<'a> OpenVRPtr for PhysicalDevice<'a> {
	type PtrType = *mut VkPhysicalDevice_T;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.internal_object().as_raw() as Self::PtrType
	}
}

impl OpenVRPtr for Device {
	type PtrType = *mut VkDevice_T;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.internal_object().as_raw() as Self::PtrType
	}
}

impl OpenVRPtr for Queue {
	type PtrType = *mut VkQueue_T;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.internal_object_guard().as_raw() as Self::PtrType
	}
}

impl OpenVRPtr for AttachmentImage {
	type PtrType = u64;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.inner().image.internal_object().as_raw()
	}
}

impl OpenVRPtr for ImmutableImage {
	type PtrType = u64;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.inner().image.internal_object().as_raw()
	}
}

impl OpenVRPtr for StorageImage {
	type PtrType = u64;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.inner().image.internal_object().as_raw()
	}
}
