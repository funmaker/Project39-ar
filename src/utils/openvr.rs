use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;
use openvr::{TrackedDevicePose, MAX_TRACKED_DEVICE_COUNT};
use openvr::compositor::WaitPoses;
use openvr_sys as sys;
use openvr_sys::{TrackedDevicePose_t, HmdMatrix34_t, HmdVector3_t, ETrackedDeviceClass_TrackedDeviceClass_Invalid};
use thiserror::Error;
use vulkano::buffer::{Buffer, BufferUsage};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, CopyImageToBufferInfo};
use vulkano::format::Format;
use vulkano::image::Image;
use vulkano::memory::allocator::MemoryTypeFilter;

use crate::application::Application;
use crate::renderer::Renderer;
use super::IntoInfo;

pub fn default_tracked_pose() -> TrackedDevicePose {
	TrackedDevicePose::from(TrackedDevicePose_t {
		mDeviceToAbsoluteTracking: HmdMatrix34_t { m: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]] },
		vVelocity: HmdVector3_t { v: [0.0, 0.0, 0.0] },
		vAngularVelocity: HmdVector3_t { v: [0.0, 0.0, 0.0] },
		eTrackingResult: ETrackedDeviceClass_TrackedDeviceClass_Invalid,
		bPoseIsValid: false,
		bDeviceIsConnected: false
	})
}

pub fn default_wait_poses() -> WaitPoses {
	WaitPoses {
		render: [default_tracked_pose(); MAX_TRACKED_DEVICE_COUNT],
		game: [default_tracked_pose(); MAX_TRACKED_DEVICE_COUNT],
	}
}

pub fn dump_camera_ss(application: &Application) -> Result<()> {
	let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
	let dir = std::env::current_dir()?.join("ss");
	fs::create_dir_all(&dir)?;

	// Capture SteamVR first, so all captures are as close in time as possible.
	let steamvr = if let Some(vr) = &application.vr {
		let _vr = vr.lock().unwrap();
		capture_steamvr(&dir, timestamp)?
	} else {
		eprintln!("No VR, skipping SteamVR screenshot");
		None
	};

	let (framebuffer, camera) = {
		let renderer = &mut *application.renderer.borrow_mut();

		let framebuffer = application.eyes
		                             .as_ref()
		                             .map(|eyes| eyes.framebuffer_image())
		                             .map(|image| read_image(renderer, image.clone()))
		                             .transpose()?;
		let camera = application.eyes
		                        .as_ref()
		                        .and_then(|eyes| eyes.camera_image())
		                        .map(|image| read_image(renderer, image.clone()))
		                        .transpose()?;

		(framebuffer, camera)
	};

	// State of the last rendered frame, matching the framebuffer and camera images above.
	let projection = application.eyes
	                            .as_ref()
	                            .map(|eyes| eyes.projection_dump());

	// PNG encoding is slow, don't stall the frame.
	thread::Builder::new()
		.name("Screenshot Dump".to_string())
		.spawn(move || {
			let result: Result<()> = try {
				if let Some(steamvr) = steamvr {
					steamvr.save(&dir.join(format!("{timestamp}-steamvr.png")))?;
				}
				if let Some(framebuffer) = framebuffer {
					framebuffer.save(&dir.join(format!("{timestamp}-project39.png")))?;
				}
				if let Some(camera) = camera {
					camera.save(&dir.join(format!("{timestamp}-camera.png")))?;
				}
				if let Some(projection) = projection {
					let toml = toml::to_string_pretty(&projection).map_err(anyhow::Error::from)?;
					fs::write(dir.join(format!("{timestamp}-projection.toml")), toml).map_err(anyhow::Error::from)?;
				}
			};

			match result {
				Ok(()) => println!("Screenshot dumped to {}", dir.join(format!("{timestamp}-*")).display()),
				Err(err) => eprintln!("Failed to save screenshot: {err:?}"),
			}
		})?;

	Ok(())
}

/// Reads the compositor's mirror textures (full resolution, with overlays) of both eyes.
#[cfg(windows)]
fn capture_steamvr(_dir: &Path, _timestamp: u128) -> Result<Option<ImageDump>> {
	use std::ffi::c_void;
	use windows::Win32::Foundation::HMODULE;
	use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
	use windows::Win32::Graphics::Direct3D11::*;

	let compositor: &sys::VR_IVRCompositor_FnTable = unsafe { &*load_fn_table(sys::IVRCompositor_Version)? };
	let get_mirror = compositor.GetMirrorTextureD3D11.ok_or(ScreenshotError::MissingFunction("GetMirrorTextureD3D11"))?;
	let release_mirror = compositor.ReleaseMirrorTextureD3D11.ok_or(ScreenshotError::MissingFunction("ReleaseMirrorTextureD3D11"))?;

	// SteamVR's compositor runs on D3D11, so a throwaway D3D11 device is the easiest way to access the mirror textures.
	let mut device = None;
	let mut context = None;
	unsafe {
		D3D11CreateDevice(None, D3D_DRIVER_TYPE_HARDWARE, HMODULE::default(), D3D11_CREATE_DEVICE_FLAG(0), None,
		                  D3D11_SDK_VERSION, Some(&mut device), None, Some(&mut context))?;
	}
	let device: ID3D11Device = device.ok_or(ScreenshotError::D3D11DeviceCreation)?;
	let context: ID3D11DeviceContext = context.ok_or(ScreenshotError::D3D11DeviceCreation)?;

	let mut views: Vec<*mut c_void> = Vec::new();
	let result = read_mirrors(&device, &context, get_mirror, &mut views);

	for view in views {
		unsafe { release_mirror(view) };
	}

	Ok(Some(result?))
}

/// Gets the mirror texture views of both eyes into `views` (released by the caller) and reads them, eyes side by side.
#[cfg(windows)]
fn read_mirrors(device: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
                context: &windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
                get_mirror: unsafe extern "C" fn(sys::EVREye, *mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> sys::EVRCompositorError,
                views: &mut Vec<*mut std::ffi::c_void>)
                -> Result<ImageDump> {
	use windows::core::Interface;
	use windows::Win32::Graphics::Direct3D11::*;

	for eye in [sys::EVREye_Eye_Left, sys::EVREye_Eye_Right] {
		let mut view = std::ptr::null_mut();
		let error = unsafe { get_mirror(eye, device.as_raw(), &mut view) };
		if error != sys::EVRCompositorError_VRCompositorError_None || view.is_null() {
			return Err(ScreenshotError::MirrorTexture(error).into());
		}
		views.push(view);
	}

	let mut textures: Vec<ID3D11Texture2D> = Vec::new();
	for view in views.iter() {
		let view = unsafe { ID3D11ShaderResourceView::from_raw_borrowed(view) }.ok_or(ScreenshotError::MirrorTexture(0))?;
		let texture: ID3D11Texture2D = unsafe { view.GetResource()? }.cast()?;

		// Some SteamVR versions return the same texture for both eyes.
		if !textures.contains(&texture) {
			textures.push(texture);
		}
	}

	let mut dumps = textures.iter()
	                        .map(|texture| read_d3d11_texture(device, context, texture))
	                        .collect::<Result<Vec<_>>>()?;

	let mut dump = dumps.remove(0);
	for other in dumps {
		if (other.width, other.height, other.format) != (dump.width, dump.height, dump.format) {
			return Err(ScreenshotError::MirrorMismatch.into());
		}
		dump.layers += other.layers;
		dump.data.extend(other.data);
	}

	Ok(dump)
}

#[cfg(windows)]
fn read_d3d11_texture(device: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
                      context: &windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
                      texture: &windows::Win32::Graphics::Direct3D11::ID3D11Texture2D)
                      -> Result<ImageDump> {
	use windows::Win32::Graphics::Direct3D11::*;
	use windows::Win32::Graphics::Dxgi::Common::*;

	let mut desc = D3D11_TEXTURE2D_DESC::default();
	unsafe { texture.GetDesc(&mut desc) };

	let format = match desc.Format {
		DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_R8G8B8A8_TYPELESS => Format::R8G8B8A8_UNORM,
		DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => Format::R8G8B8A8_SRGB,
		DXGI_FORMAT_B8G8R8A8_UNORM | DXGI_FORMAT_B8G8R8A8_TYPELESS => Format::B8G8R8A8_UNORM,
		DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => Format::B8G8R8A8_SRGB,
		format => return Err(ScreenshotError::UnsupportedDxgiFormat(format.0).into()),
	};

	let staging_desc = D3D11_TEXTURE2D_DESC {
		MipLevels: 1,
		ArraySize: 1,
		SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
		Usage: D3D11_USAGE_STAGING,
		BindFlags: 0,
		CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
		MiscFlags: 0,
		..desc
	};

	let mut staging = None;
	unsafe { device.CreateTexture2D(&staging_desc, None, Some(&mut staging))? };
	let staging = staging.ok_or(ScreenshotError::MirrorTexture(0))?;

	unsafe { context.CopySubresourceRegion(&staging, 0, 0, 0, 0, texture, 0, None) };

	let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
	unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))? };

	let row = desc.Width as usize * 4;
	let mut data = Vec::with_capacity(row * desc.Height as usize);
	for y in 0..desc.Height as usize {
		let src = unsafe { std::slice::from_raw_parts((mapped.pData as *const u8).add(y * mapped.RowPitch as usize), row) };
		data.extend_from_slice(src);
	}

	unsafe { context.Unmap(&staging, 0) };

	Ok(ImageDump { width: desc.Width, height: desc.Height, layers: 1, format, data })
}

/// Fallback: SteamVR's stereo screenshot, saved by SteamVR itself (cropped and without overlays).
#[cfg(not(windows))]
fn capture_steamvr(dir: &Path, timestamp: u128) -> Result<Option<ImageDump>> {
	// SteamVR appends the extension itself.
	take_stereo_screenshot(&dir.join(format!("{timestamp}-steamvr-preview")),
	                       &dir.join(format!("{timestamp}-steamvr")))?;

	Ok(None)
}

/// Loads OpenVR's C function table of interface `version` (e.g. `openvr_sys::IVRSystem_Version`).
pub fn load_fn_table<T>(version: &[u8]) -> Result<*const T> {
	let mut magic = Vec::from(b"FnTable:".as_ref());
	magic.extend(version);
	let mut error = sys::EVRInitError_VRInitError_None;
	let fn_table = unsafe { sys::VR_GetGenericInterface(magic.as_ptr() as *const _, &mut error) as *const T };
	if error != sys::EVRInitError_VRInitError_None || fn_table.is_null() {
		return Err(ScreenshotError::InterfaceNotFound(error).into());
	}

	Ok(fn_table)
}

#[cfg(not(windows))]
fn take_stereo_screenshot(preview_path: &Path, vr_path: &Path) -> Result<()> {
	use std::ffi::CString;
	use anyhow::anyhow;

	let to_cstring = |path: &Path| CString::new(path.to_str().ok_or_else(|| anyhow!("Non UTF-8 path: {}", path.display()))?)
		.map_err(anyhow::Error::from);
	let preview_path = to_cstring(preview_path)?;
	let vr_path = to_cstring(vr_path)?;

	let fn_table: &sys::VR_IVRScreenshots_FnTable = unsafe { &*load_fn_table(sys::IVRScreenshots_Version)? };
	let take = fn_table.TakeStereoScreenshot.ok_or(ScreenshotError::MissingFunction("TakeStereoScreenshot"))?;

	let mut handle: sys::ScreenshotHandle_t = 0;
	let result = unsafe { take(&mut handle, preview_path.as_ptr() as *mut _, vr_path.as_ptr() as *mut _) };
	if result != sys::EVRScreenshotError_VRScreenshotError_None {
		return Err(ScreenshotError::Failed(result).into());
	}

	Ok(())
}

struct ImageDump {
	width: u32,
	height: u32,
	layers: u32,
	format: Format,
	data: Vec<u8>,
}

impl ImageDump {
	/// Saves the image as RGBA PNG, with array layers placed side by side.
	fn save(&self, path: &Path) -> Result<()> {
		let swap_rb = match self.format {
			Format::R8G8B8A8_SRGB | Format::R8G8B8A8_UNORM => false,
			Format::B8G8R8A8_SRGB | Format::B8G8R8A8_UNORM => true,
			format => return Err(ScreenshotError::UnsupportedFormat(format).into()),
		};

		let (width, height, layers) = (self.width as usize, self.height as usize, self.layers as usize);
		let row = width * 4;
		let mut out = vec![0; row * layers * height];

		for layer in 0..layers {
			for y in 0..height {
				let src = &self.data[(layer * height + y) * row..][..row];
				let dst = &mut out[y * row * layers + layer * row..][..row];

				for (src, dst) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
					if swap_rb {
						dst.copy_from_slice(&[src[2], src[1], src[0], 255]);
					} else {
						dst.copy_from_slice(&[src[0], src[1], src[2], 255]);
					}
				}
			}
		}

		image::save_buffer(path, &out, self.width * self.layers, self.height, image::ColorType::Rgba8)?;

		Ok(())
	}
}

/// Copies the whole image (all array layers) to the host, waiting for all previously enqueued GPU work.
fn read_image(renderer: &mut Renderer, image: Arc<Image>) -> Result<ImageDump> {
	let [width, height, _] = image.extent();
	let layers = image.array_layers();
	let format = image.format();

	let buffer = Buffer::new_slice::<u8>(renderer.memory_allocator.clone(),
	                                     BufferUsage::TRANSFER_DST.into_info(),
	                                     (MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_RANDOM_ACCESS).into_info(),
	                                     width as u64 * height as u64 * layers as u64 * format.block_size())?;

	let mut builder = AutoCommandBufferBuilder::primary(renderer.command_buffer_allocator.clone(), renderer.queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit)?;
	builder.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(image, buffer.clone()))?;
	let command_buffer = builder.build()?;

	renderer.execute_and_wait(renderer.queue.clone(), command_buffer)?;

	let data = buffer.read()?.to_vec();

	Ok(ImageDump { width, height, layers, format, data })
}

#[derive(Debug, Error)]
pub enum ScreenshotError {
	#[error("Unable to get OpenVR interface ({0})")] InterfaceNotFound(sys::EVRInitError),
	#[error("OpenVR function {0} is missing")] MissingFunction(&'static str),
	#[cfg(not(windows))]
	#[error("TakeStereoScreenshot failed ({0})")] Failed(sys::EVRScreenshotError),
	#[error("Unable to get compositor mirror texture ({0})")] MirrorTexture(sys::EVRCompositorError),
	#[error("Mirror textures of both eyes have different sizes or formats")] MirrorMismatch,
	#[error("Failed to create D3D11 device")] D3D11DeviceCreation,
	#[error("Unsupported DXGI format {0}")] UnsupportedDxgiFormat(i32),
	#[error("Unsupported image format {0:?}")] UnsupportedFormat(Format),
}
