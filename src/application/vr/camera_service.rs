use std::sync::Arc;
use openvr::{TrackedDeviceIndex, TrackedDevicePose};

use super::{VR, TrackedCameraError, FrameType};
use super::tracked_camera::TrackedCameraHandle;


pub struct CameraService {
	vr: Arc<VR>,
	index: TrackedDeviceIndex,
	handle: TrackedCameraHandle,
	frame_buffer: Option<FrameBuffer>,
}

impl CameraService {
	pub fn new(vr: Arc<VR>, index: TrackedDeviceIndex) -> Result<CameraService, TrackedCameraError> {
		let handle = unsafe { vr.lock().unwrap().tracked_camera.acquire_video_streaming_service(index)? };
		
		Ok(CameraService {
			vr,
			index,
			handle,
			frame_buffer: None,
		})
	}
	
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
			standing_device_pose: header.standingTrackedDevicePose.into(),
			frame_exposure_time: header.ulFrameExposureTime,
			buffer,
		});
		
		Ok(self.frame_buffer.as_mut().unwrap())
	}
	
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

pub struct FrameBuffer {
	pub frame_type: FrameType,
	pub width: u32,
	pub height: u32,
	pub bytes_per_pixel: u32,
	pub frame_sequence: u32,
	pub standing_device_pose: TrackedDevicePose,
	pub frame_exposure_time: u64,
	pub buffer: Vec<u8>,
}
