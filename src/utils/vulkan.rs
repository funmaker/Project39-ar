use std::ffi::CString;
use openvr::{VkInstance_T, VkPhysicalDevice_T, Compositor, VkDevice_T, VkQueue_T};
use vulkano::instance::Instance;
use vulkano::{VulkanObject, SynchronizedVulkanObject, Handle};
use vulkano::device::{Device, Queue};
use vulkano::device::physical::PhysicalDevice;
use vulkano::image::{AttachmentImage, ImageAccess, StorageImage, ImmutableImage};

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

