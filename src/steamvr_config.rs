use std::path::Path;
use std::fs::File;
use err_derive::Error;
use serde::{Deserialize, Serialize};

use crate::config::{CameraConfig, CameraEyeConfig, CameraAPI};
use crate::math::{IVec2, Vec2, Vec3, Vec4};

pub fn load_steamvr_config(path: impl AsRef<Path>) -> Result<CameraConfig, CameraConfigLoadError> {
	let config: HMDConfig = serde_json::from_reader(File::open(path)?)?;
	
	Ok(CameraConfig {
		driver: CameraAPI::Dummy,
		id: 1337,
		frame_buffer_size: IVec2::new(1920, 960),
		device_serial_number: config.device_serial_number,
		left: CameraEyeConfig {
			offset: IVec2::new(0, 0),
			size: IVec2::new(config.tracked_cameras[0].intrinsics.width, config.tracked_cameras[0].intrinsics.height),
			focal_length: Vec2::new(config.tracked_cameras[0].intrinsics.focal_x, config.tracked_cameras[0].intrinsics.focal_y),
			center: Vec2::new(config.tracked_cameras[0].intrinsics.center_x, config.tracked_cameras[0].intrinsics.center_y),
			coeffs: Vec4::from(config.tracked_cameras[0].intrinsics.distort.coeffs),
			position: Vec3::from(config.tracked_cameras[0].extrinsics.position),
			right: Vec3::from(config.tracked_cameras[0].extrinsics.plus_x),
			back: Vec3::from(config.tracked_cameras[0].extrinsics.plus_z),
			cal_method: config.tracked_cameras[0].cal_method.clone(),
			name: config.tracked_cameras[0].name.clone(),
			white_balance: Vec4::from(config.tracked_cameras[0].white_balance),
		},
		right: CameraEyeConfig {
			offset: IVec2::new(config.tracked_cameras[0].intrinsics.width, 0),
			size: IVec2::new(config.tracked_cameras[1].intrinsics.width, config.tracked_cameras[1].intrinsics.height),
			focal_length: Vec2::new(config.tracked_cameras[1].intrinsics.focal_x, config.tracked_cameras[1].intrinsics.focal_y),
			center: Vec2::new(config.tracked_cameras[1].intrinsics.center_x, config.tracked_cameras[1].intrinsics.center_y),
			coeffs: Vec4::from(config.tracked_cameras[1].intrinsics.distort.coeffs),
			position: Vec3::from(config.tracked_cameras[1].extrinsics.position),
			right: Vec3::from(config.tracked_cameras[1].extrinsics.plus_x),
			back: Vec3::from(config.tracked_cameras[1].extrinsics.plus_z),
			cal_method: config.tracked_cameras[1].cal_method.clone(),
			name: config.tracked_cameras[1].name.clone(),
			white_balance: Vec4::from(config.tracked_cameras[1].white_balance),
		}
	})
}

#[derive(Serialize, Deserialize)]
struct HMDConfig {
	device_serial_number: String,
	tracked_cameras: [TrackedCamera; 2],
}

#[derive(Serialize, Deserialize)]
struct TrackedCamera {
	cal_method: String,
	extrinsics: Extrinsics,
	intrinsics: Intrinsics,
	name: String,
	white_balance: [f32; 4],
}

#[derive(Serialize, Deserialize)]
struct Extrinsics {
	plus_x: [f32; 3],
	plus_z: [f32; 3],
	position: [f32; 3],
}

#[derive(Serialize, Deserialize)]
struct Intrinsics {
	center_x: f32,
	center_y: f32,
	distort: Distort,
	focal_x: f32,
	focal_y: f32,
	height: i32,
	interface: String,
	width: i32,
}

#[derive(Serialize, Deserialize)]
struct Distort {
	coeffs: [f32; 4],
	#[serde(rename = "type")] dtype: String,
}

#[derive(Debug, Error)]
pub enum CameraConfigLoadError {
	#[error(display = "{}", _0)] IOError(#[error(source)] std::io::Error),
	#[error(display = "{}", _0)] Infallible(#[error(source)] std::convert::Infallible),
	#[error(display = "{}", _0)] SerdeError(#[error(source)] serde_json::Error),
}
