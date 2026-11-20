use serde::Serialize;

use crate::config::CameraConfig;
use crate::math::{Isometry3, Mat4};


/// Everything needed to reproduce the passthrough projection (`eyes/pipeline/{vert,frag}.glsl`) offline.
/// All matrices are column-major (each inner array is a column, like GLSL), poses are in tracking space (standing space in VR).
#[derive(Serialize, Debug, Clone)]
pub struct ProjectionDump {
	/// Size of each eye's framebuffer (`<ts>-project39.png` has both eyes side by side).
	pub framebuffer_size: [u32; 2],
	/// Size of the camera image (`<ts>-camera.png`).
	pub camera_frame_size: [u32; 2],
	/// `camera.frame_buffer_size`.
	pub camera_frame_buffer_size: [i32; 2],
	/// HMD pose submitted with the frame (OpenVR row-major 3x4 device to absolute tracking).
	pub hmd_pose_row_major: [[f32; 4]; 3],
	pub eyes: EyesDump,
	pub background: Option<BackgroundDump>,
	pub openvr: Option<OpenVRDump>,
	/// `[camera]` section of the config.
	pub camera_config: CameraConfig,
}

#[derive(Serialize, Debug, Clone)]
pub struct EyesDump {
	/// View matrix (head to eye) of each eye.
	pub view: [[[f32; 4]; 4]; 2],
	/// Projection (clip space with Vulkan correction) of each eye.
	pub projection: [[[f32; 4]; 4]; 2],
	/// Tangents of half fov per eye: [left, right, top, bottom].
	pub raw_projection: [[f32; 4]; 2],
}

#[derive(Serialize, Debug, Clone)]
pub struct BackgroundDump {
	/// Head pose (tracking space) when the last camera frame was captured.
	pub camera_frame_pose: PoseDump,
	/// Head pose (tracking space) used for the last background render.
	pub render_head_pose: Option<PoseDump>,
	/// Uniform buffer (`pipeline/params.glsl`) of the last background render, exactly as uploaded.
	pub params: Option<ParamsDump>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ParamsDump {
	pub eye_to_tracking: [[[f32; 4]; 4]; 2],
	pub tracking_to_camera: [[[f32; 4]; 4]; 2],
	pub rawproj: [[f32; 4]; 2],
	pub coeffs: [[f32; 4]; 2],
	pub focal_center: [[f32; 4]; 2],
	pub bounds: [[f32; 4]; 2],
	pub tan_shift: [[f32; 4]; 2],
	pub raw_to_tracking: [[f32; 4]; 4],
pub dome: [f32; 4],
	pub frame_size: [f32; 4],
}

/// Values reported by OpenVR.
#[derive(Serialize, Debug, Clone)]
pub struct OpenVRDump {
	/// HMD camera to head transform (row-major 3x4).
	pub camera_to_head_row_major: Option<[[f32; 4]; 3]>,
	/// Distorted camera frame size (the frame type used by the openvr camera driver).
	pub distorted_frame_size: Option<[u32; 2]>,
	/// Per camera: [focal_x, focal_y, center_x, center_y] in pixels.
	pub intrinsics: Vec<[f32; 4]>,
	/// Per eye: eye to head transform (row-major 3x4).
	pub eye_to_head_row_major: [[[f32; 4]; 3]; 2],
	/// Per eye: projection_raw [left, right, top, bottom].
	pub projection_raw: [[f32; 4]; 2],
}

#[derive(Serialize, Debug, Clone)]
pub struct PoseDump {
	pub translation: [f32; 3],
	/// Quaternion [x, y, z, w].
	pub rotation: [f32; 4],
	pub matrix: [[f32; 4]; 4],
}

impl From<Isometry3> for PoseDump {
	fn from(pose: Isometry3) -> Self {
		PoseDump {
			translation: pose.translation.vector.into(),
			rotation: pose.rotation.coords.into(),
			matrix: pose.to_homogeneous().into(),
		}
	}
}

pub fn mat4(mat: &Mat4) -> [[f32; 4]; 4] {
	(*mat).into()
}
