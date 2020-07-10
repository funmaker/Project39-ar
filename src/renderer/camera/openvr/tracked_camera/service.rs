use std::{ptr, mem};

use openvr_sys as sys;
use openvr::{TrackedDeviceIndex, TrackedDevicePose};

use super::{check_err, TrackedCameraError, TrackedCamera, FrameType};

pub struct CameraService {
	camera: TrackedCamera,
	index: TrackedDeviceIndex,
	handle: sys::TrackedCameraHandle_t,
	frame_buffer: Option<FrameBuffer>,
}

impl CameraService {
	pub fn new(camera: TrackedCamera, index: TrackedDeviceIndex) -> Result<Self, TrackedCameraError> {
		let mut handle = 0;
		
		check_err(camera.0, unsafe {
			camera.0.AcquireVideoStreamingService.unwrap()(index,
			                                               &mut handle)
		})?;
		
		Ok(CameraService {
			camera,
			index,
			handle,
			frame_buffer: None,
		})
	}
	
	pub fn get_frame_buffer(&mut self, frame_type: FrameType) -> Result<&FrameBuffer, TrackedCameraError> {
		let mut buffer = self.frame_buffer
		                     .take()
		                     .filter(|fb| fb.frame_type == frame_type)
		                     .map(|fb| Ok(fb.buffer))
		                     .unwrap_or_else(|| self.new_buffer(frame_type))?;
		
		let mut header = sys::CameraVideoStreamFrameHeader_t {
			eFrameType: frame_type.into(),
			nWidth: 0,
			nHeight: 0,
			nBytesPerPixel: 0,
			nFrameSequence: 0,
			standingTrackedDevicePose: sys::TrackedDevicePose_t {
				mDeviceToAbsoluteTracking: sys::HmdMatrix34_t { m: [[0.0; 4]; 3] },
				vVelocity: sys::HmdVector3_t { v: [0.0, 0.0, 0.0] },
				vAngularVelocity: sys::HmdVector3_t { v: [0.0, 0.0, 0.0] },
				eTrackingResult: 0,
				bPoseIsValid: false,
				bDeviceIsConnected: false
			},
			ulFrameExposureTime: 0
		};
		
		check_err(self.camera.0, unsafe {
			self.camera.0.GetVideoStreamFrameBuffer.unwrap()(self.handle,
			                                                 frame_type.into(),
			                                                 buffer.as_mut_ptr() as *mut _,
			                                                 buffer.len() as u32,
			                                                 &mut header,
			                                                 mem::size_of::<sys::CameraVideoStreamFrameHeader_t>() as u32)
		})?;
		
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
		
		Ok(self.frame_buffer.as_ref().unwrap())
	}
	
	fn new_buffer(&self, frame_type: FrameType) -> Result<Vec<u8>, TrackedCameraError> {
		let mut frame_buffer_size = 0;
		
		check_err(self.camera.0, unsafe {
			self.camera.0.GetCameraFrameSize.unwrap()(self.index,
			                                          frame_type.into(),
			                                          ptr::null_mut(),
			                                          ptr::null_mut(),
			                                          &mut frame_buffer_size)
		})?;
		
		Ok(vec![0; frame_buffer_size as usize])
	}
}

impl Drop for CameraService {
	fn drop(&mut self) { unsafe {
		check_err(self.camera.0, self.camera.0.ReleaseVideoStreamingService.unwrap()(self.handle))
			.expect("Unable to release video streaming service");
	}}
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

