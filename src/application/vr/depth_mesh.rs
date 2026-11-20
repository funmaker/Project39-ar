use std::ffi::{c_char, c_void, CString};
use anyhow::Result;
use thiserror::Error;
use openvr_sys as sys;


const PREFIX: &str = "/lighthouse/depthAugmentedPassthrough/";

/// Reads the depth mesh SteamVR's lighthouse driver produces for its 3D room view ("depth augmented passthrough").
/// Uses SteamVR's internal (undocumented) block queue and path property interfaces, the same way vrcompositor does.
///
/// The mesh is a `width` x `height` grid of points (raw tracking space) sampled every 4 pixels of the left
/// camera's frame, triangulated as a regular grid. Invalid points are NaN.
pub struct DepthMesh {
	block_queue: *mut c_void,
	paths: *mut c_void,
	queue: u64,
	width: u32,
	height: u32,
	original_mode: i32,
	enabled: bool,
	last_mesh_id: Option<u64>,
}

impl !Sync for DepthMesh {}
impl !Send for DepthMesh {}

/// Mesh data copied out of the queue.
pub struct DepthMeshFrame {
	pub width: u32,
	pub height: u32,
	/// Left camera mesh, `width * height` points. The block can also contain a right camera mesh,
	/// but the Index's driver doesn't produce one and SteamVR then uses the left one for both eyes.
	pub left: Vec<[f32; 3]>,
}

#[repr(C)]
struct PathRead {
	path: u64,
	buffer: *mut c_void,
	size: u32,
	tag: u32,
	required: u32,
	error: i32,
	path_str: *const c_char,
}

#[repr(C)]
struct PathWrite {
	path: u64,
	write_type: i32,
	set_error: i32,
	buffer: *mut c_void,
	size: u32,
	tag: u32,
	error: i32,
	path_str: *const c_char,
}

// IVRBlockQueue_005
type ConnectFn = unsafe extern "C" fn(*mut c_void, *mut u64, *const c_char) -> i32;
type AcquireReadOnlyBlockFn = unsafe extern "C" fn(*mut c_void, u64, *mut u64, *mut *const c_void, i32) -> i32;
type ReleaseReadOnlyBlockFn = unsafe extern "C" fn(*mut c_void, u64, u64) -> i32;
const CONNECT: usize = 1;
const ACQUIRE_READ_ONLY_BLOCK: usize = 6;
const RELEASE_READ_ONLY_BLOCK: usize = 7;
const READ_LATEST: i32 = 0;

// IVRPaths_002
type ReadPathBatchFn = unsafe extern "C" fn(*mut c_void, u64, *mut PathRead, u32) -> i32;
type WritePathBatchFn = unsafe extern "C" fn(*mut c_void, u64, *mut PathWrite, u32) -> i32;
type StringToHandleFn = unsafe extern "C" fn(*mut c_void, *mut u64, *const c_char) -> i32;
const READ_PATH_BATCH: usize = 0;
const WRITE_PATH_BATCH: usize = 1;
const STRING_TO_HANDLE: usize = 2;

const TAG_INT32: u32 = sys::k_unInt32PropertyTag as u32;

/// Safety: `obj` must be a valid C++ interface pointer whose vtable has a function of type `F` at `index`.
unsafe fn vfn<F: Copy>(obj: *mut c_void, index: usize) -> F {
	unsafe {
		let vtable = *(obj as *const *const usize);
		std::mem::transmute_copy(&*vtable.add(index))
	}
}

fn interface(name: &str) -> Result<*mut c_void> {
	let name = CString::new(name)?;
	let mut error = sys::EVRInitError_VRInitError_None;
	let ptr = unsafe { sys::VR_GetGenericInterface(name.as_ptr(), &mut error) };
	if error != sys::EVRInitError_VRInitError_None || ptr == 0 {
		return Err(DepthMeshError::Interface(name.into_string()?, error).into());
	}
	Ok(ptr as *mut c_void)
}

impl DepthMesh {
	/// OpenVR has to be initialized.
	pub fn new() -> Result<DepthMesh> {
		let block_queue = interface("IVRBlockQueue_005")?;
		let paths = interface("IVRPaths_002")?;

		let mut queue = 0;
		let path = CString::new(format!("{PREFIX}rawMesh"))?;
		let error = unsafe { vfn::<ConnectFn>(block_queue, CONNECT)(block_queue, &mut queue, path.as_ptr()) };
		if error != 0 {
			return Err(DepthMeshError::Connect(error).into());
		}

		let mut mesh = DepthMesh {
			block_queue,
			paths,
			queue,
			width: 0,
			height: 0,
			original_mode: 0,
			enabled: false,
			last_mesh_id: None,
		};

		mesh.width = mesh.read_i32(queue, "meshWidth")?.max(0) as u32;
		mesh.height = mesh.read_i32(queue, "meshHeight")?.max(0) as u32;
		mesh.original_mode = mesh.read_i32(queue, "processingMode")?;

		Ok(mesh)
	}

	/// Turns the driver's depth processing on (it's off unless SteamVR's 3D room view is used).
	pub fn set_enabled(&mut self, enabled: bool) -> Result<()> {
		if enabled != self.enabled {
			self.write_i32(self.queue, "processingMode", if enabled { 1 } else { self.original_mode })?;
			self.enabled = enabled;
			self.last_mesh_id = None;
		}

		Ok(())
	}

	/// Returns the latest mesh if it changed since the last call.
	pub fn poll(&mut self) -> Result<Option<DepthMeshFrame>> {
		if !self.enabled || self.width == 0 || self.height == 0 {
			return Ok(None);
		}

		let mut block = 0;
		let mut buffer: *const c_void = std::ptr::null();
		let error = unsafe { vfn::<AcquireReadOnlyBlockFn>(self.block_queue, ACQUIRE_READ_ONLY_BLOCK)(self.block_queue, self.queue, &mut block, &mut buffer, READ_LATEST) };
		if error != 0 || buffer.is_null() {
			return Ok(None);
		}

		let result = self.read_block(block, buffer);

		unsafe { vfn::<ReleaseReadOnlyBlockFn>(self.block_queue, RELEASE_READ_ONLY_BLOCK)(self.block_queue, self.queue, block); }

		result
	}

	fn read_block(&mut self, block: u64, buffer: *const c_void) -> Result<Option<DepthMeshFrame>> {
		let mesh_id = self.read_u64(block, "meshId")?;
		if self.last_mesh_id == Some(mesh_id) {
			return Ok(None);
		}
		self.last_mesh_id = Some(mesh_id);

		let points = (self.width * self.height) as usize;
		let left_count = self.read_i32(block, "leftMeshElementCount")?.max(0) as usize; // floats
		if left_count < points * 3 {
			return Ok(None);
		}
		
		let floats = unsafe { std::slice::from_raw_parts(buffer as *const f32, points * 3) };
		
		Ok(Some(DepthMeshFrame {
			width: self.width,
			height: self.height,
			left: floats.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect(),
		}))
	}

	fn handle(&self, name: &str) -> Result<u64> {
		let path = CString::new(format!("{PREFIX}{name}"))?;
		let mut handle = 0;
		let error = unsafe { vfn::<StringToHandleFn>(self.paths, STRING_TO_HANDLE)(self.paths, &mut handle, path.as_ptr()) };
		if error != 0 {
			return Err(DepthMeshError::Property(name.to_string(), error).into());
		}
		Ok(handle)
	}

	fn read<T: Default>(&self, container: u64, name: &str) -> Result<T> {
		let mut value = T::default();
		let mut read = PathRead {
			path: self.handle(name)?,
			buffer: &mut value as *mut T as *mut c_void,
			size: size_of::<T>() as u32,
			tag: 0,
			required: 0,
			error: 0,
			path_str: std::ptr::null(),
		};
		let error = unsafe { vfn::<ReadPathBatchFn>(self.paths, READ_PATH_BATCH)(self.paths, container, &mut read, 1) };
		if error != 0 || read.error != 0 {
			return Err(DepthMeshError::Property(name.to_string(), if error != 0 { error } else { read.error }).into());
		}
		Ok(value)
	}

	fn read_i32(&self, container: u64, name: &str) -> Result<i32> {
		self.read(container, name)
	}

	fn read_u64(&self, container: u64, name: &str) -> Result<u64> {
		self.read(container, name)
	}

	fn write_i32(&self, container: u64, name: &str, mut value: i32) -> Result<()> {
		let mut write = PathWrite {
			path: self.handle(name)?,
			write_type: 0,
			set_error: 0,
			buffer: &mut value as *mut i32 as *mut c_void,
			size: 4,
			tag: TAG_INT32,
			error: 0,
			path_str: std::ptr::null(),
		};
		let error = unsafe { vfn::<WritePathBatchFn>(self.paths, WRITE_PATH_BATCH)(self.paths, container, &mut write, 1) };
		if error != 0 || write.set_error != 0 {
			return Err(DepthMeshError::Property(name.to_string(), if error != 0 { error } else { write.set_error }).into());
		}
		Ok(())
	}
}

impl Drop for DepthMesh {
	fn drop(&mut self) {
		if let Err(err) = self.set_enabled(false) {
			eprintln!("Failed to restore depth augmented passthrough processing mode: {err}");
		}
	}
}

#[derive(Debug, Error)]
pub enum DepthMeshError {
	#[error("Unable to get OpenVR interface {0} ({1})")] Interface(String, sys::EVRInitError),
	#[error("Failed to connect to the depth mesh block queue ({0})")] Connect(i32),
	#[error("Failed to access depth mesh property {0} ({1})")] Property(String, i32),
}
