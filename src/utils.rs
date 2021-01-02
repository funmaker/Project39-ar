use std::ffi::CString;
use vulkano::instance::{Instance, PhysicalDevice};
use vulkano::{VulkanObject, SynchronizedVulkanObject};
use vulkano::device::{Device, Queue};
use vulkano::image::{AttachmentImage, ImageAccess};
use openvr::{VkInstance_T, VkPhysicalDevice_T, Compositor, VkDevice_T, VkQueue_T};
use cgmath::{Matrix4, Matrix, Decomposed, Quaternion, Vector3, Zero, Matrix3, InnerSpace};
use image::DynamicImage;

// Math

pub fn mat4(val: &[[f32; 4]; 3]) -> Matrix4<f32> {
	let mat: Matrix4<f32> = [val[0], val[1], val[2], [0.0, 0.0, 0.0, 1.0]].into();
	mat.transpose()
}

pub fn decompose(mut mat: Matrix4<f32>) -> Decomposed<Vector3<f32>, Quaternion<f32>> {
	let disp = mat.w.truncate();
	mat.w.set_zero();
	
	let scale = Vector3::new(mat.x.magnitude(), mat.y.magnitude(), mat.z.magnitude());
	
	mat.x /= scale.x;
	mat.y /= scale.y;
	mat.z /= scale.z;
	
	let rot: Quaternion<f32> = Matrix3::from_cols(mat.x.truncate(), mat.y.truncate(), mat.z.truncate()).into();
	
	Decomposed {
		scale: scale.magnitude(),
		rot,
		disp,
	}
}

pub fn mat34(val: Matrix4<f32>) -> [[f32; 4]; 3] {
	[[val.x.x, val.y.x, val.z.x, val.w.x],
	 [val.x.y, val.y.y, val.z.y, val.w.y],
	 [val.x.z, val.y.z, val.z.z, val.w.z]]
}

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
		self.internal_object() as Self::PtrType
	}
}

impl<'a> OpenVRPtr for PhysicalDevice<'a> {
	type PtrType = *mut VkPhysicalDevice_T;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.internal_object() as Self::PtrType
	}
}

impl<'a> OpenVRPtr for Device {
	type PtrType = *mut VkDevice_T;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.internal_object() as Self::PtrType
	}
}

impl<'a> OpenVRPtr for Queue {
	type PtrType = *mut VkQueue_T;
	
	fn as_ptr(&self) -> Self::PtrType {
		*self.internal_object_guard() as Self::PtrType
	}
}

impl<F: 'static + Send + Sync> OpenVRPtr for AttachmentImage<F> {
	type PtrType = u64;
	
	fn as_ptr(&self) -> Self::PtrType {
		self.inner().image.internal_object() as Self::PtrType
	}
}
