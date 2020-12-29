use std::ffi::CString;
use vulkano::instance::{Instance, PhysicalDevice};
use vulkano::{VulkanObject, SynchronizedVulkanObject};
use vulkano::device::{Device, Queue};
use vulkano::image::{AttachmentImage, ImageAccess};
use openvr::{VkInstance_T, VkPhysicalDevice_T, Compositor, VkDevice_T, VkQueue_T};
use cgmath::{Matrix4, Matrix, Decomposed, Quaternion, Vector3, Zero, Matrix3, InnerSpace};

pub fn vulkan_device_extensions_required(compositor: &Compositor, physical: &PhysicalDevice) -> Vec<CString> {
	unsafe { compositor.vulkan_device_extensions_required(physical.as_ptr()) }
}

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
