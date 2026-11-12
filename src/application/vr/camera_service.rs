use std::sync::Arc;
use openvr::{TrackedDeviceIndex, TrackedDevicePose};

use super::{VR, FrameType};
use super::tracked_camera::{TrackedCameraError, TrackedCameraHandle};


/// An acquired OpenVR video streaming service for a tracked camera. Released on drop.
pub struct CameraService {
	vr: Arc<VR>,
	index: TrackedDeviceIndex,
	handle: TrackedCameraHandle,
	frame_buffer: Option<FrameBuffer>,
}

impl CameraService {
	/// Acquires the video streaming service of the camera on the given tracked device.
	pub fn new(vr: Arc<VR>, index: TrackedDeviceIndex) -> Result<CameraService, TrackedCameraError> {
		let handle = unsafe { vr.lock().unwrap().tracked_camera.acquire_video_streaming_service(index)? };
		
		Ok(CameraService {
			vr,
			index,
			handle,
			frame_buffer: None,
		})
	}
	
	/// Fetches the latest camera frame of the given type, reusing the previous buffer when possible.
	pub fn get_frame_buffer(&mut self, frame_type: FrameType) -> Result<&mut FrameBuffer, TrackedCameraError> {
		let mut buffer = self.frame_buffer
		                     .take()
		                     .filter(|fb| fb.frame_type == frame_type)
		                     .map(|fb| Ok(fb.buffer))
		                     .unwrap_or_else(|| self.new_buffer(frame_type))?;
		
		let header = unsafe { self.vr.lock().unwrap().tracked_camera.get_video_stream_frame_buffer(self.handle, frame_type, &mut buffer)? };
		
		self.frame_buffer = Some(FrameBuffer {
			frame_type: header.eFrameType.into(),
			width: header.nWidth,
			height: header.nHeight,
			bytes_per_pixel: header.nBytesPerPixel,
			frame_sequence: header.nFrameSequence,
			standing_device_pose: header.trackedDevicePose.into(),
			frame_exposure_time: header.ulFrameExposureTime,
			buffer,
		});
		
		Ok(self.frame_buffer.as_mut().unwrap())
	}
	
	/// Allocates a buffer large enough for a frame of the given type.
	fn new_buffer(&self, frame_type: FrameType) -> Result<Vec<u8>, TrackedCameraError> {
		let frame_size = self.vr.lock().unwrap().tracked_camera.get_camera_frame_size(self.index, frame_type)?;
		
		Ok(vec![0; frame_size.frame_buffer_size as usize])
	}
}

impl Drop for CameraService {
	fn drop(&mut self) {
		unsafe { self.vr.lock().unwrap().tracked_camera.release_video_streaming_service(self.handle).expect("Unable to release video streaming service"); }
	}
}

/// A camera frame along with its OpenVR frame header data.
#[allow(dead_code)]
pub struct FrameBuffer {
	pub frame_type: FrameType,
	pub width: u32,
	pub height: u32,
	pub bytes_per_pixel: u32,
	pub frame_sequence: u32,
	/// Pose of the camera's device at the time the frame was captured.
	pub standing_device_pose: TrackedDevicePose,
	pub frame_exposure_time: u64,
	pub buffer: Vec<u8>,
}
