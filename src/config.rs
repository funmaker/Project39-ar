use std::ops::Deref;
use std::sync::Arc;
use std::str::FromStr;
use std::fmt::{Display, Formatter};
use std::env;
use serde_derive::{Deserialize, Serialize};
use arc_swap::ArcSwap;
use getopts::{Options, Matches};

use crate::utils::from_args::{FromArgs, ArgsError, args_terminals};
use crate::math::{IVec2, Vec2, Vec4, Vec3};

#[derive(Deserialize, Serialize, Debug, Clone, FromArgs)]
pub struct Config {
	/// Prints this message.
	#[serde(skip)] #[arg_short = "h"] pub help: bool,
	/// Prints an example config.
	#[serde(skip)] pub example_config: bool,
	/// Show debug info.
	#[arg_short = "d"] pub debug: bool,
	/// Enable validation layers.
	#[arg_short = "v"] pub validation: bool,
	/// Fallback GPU device to use.
	pub gpu_id: usize,
	/// Super-Sampling Anti-Aliasing factor.
	pub ssaa: f32,
	/// Multi-Sampling Anti-Aliasing factor.
	pub msaa: u32,
	/// Camera configuration
	pub camera: CameraConfig,
	/// Non VR mode
	pub novr: NovrConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone, FromArgs)]
pub struct CameraConfig {
	/// Select background API to use: escapi, opencv, openvr or dummy.
	#[arg_short = "c"] #[arg_rename = ""] pub driver: CameraAPI,
	/// Camera device index. Ignored if openvr is used.
	pub id: usize,
	/// Size of the whole frame buffer.
	#[serde(skip)] #[arg_skip] pub frame_buffer_size: IVec2,
	/// Left camera eye
	pub left: CameraEyeConfig,
	/// Right camera eye
	pub right: CameraEyeConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone, FromArgs)]
pub struct CameraEyeConfig {
	/// Frame offset on the frame buffer.
	pub offset: IVec2,
	/// Per eye frame size.
	pub size: IVec2,
	/// Focal length.
	pub focal_length: Vec2,
	/// Principal point.
	pub center: Vec2,
	/// Distortion coefficients.
	pub coeffs: Vec4,
	/// Position relative to HMD
	pub position: Vec3,
	/// Camera's right (X+)
	pub right: Vec3,
	/// Camera's back (Z+)
	pub back: Vec3,
}

#[derive(Deserialize, Serialize, Debug, Clone, FromArgs)]
pub struct NovrConfig {
	/// Enable Non VR mode. The program will not use OpenVR. Use Keyboard and mouse to move.
	#[arg_rename = ""] pub enabled: bool,
	/// Emulated output width. (one eye)
	pub frame_buffer_width: u32,
	/// Emulated output height.
	pub frame_buffer_height: u32,
	/// Emulated fov
	pub fov: f32,
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CameraAPI {
	#[cfg(windows)] Escapi,
	OpenCV,
	OpenVR,
	Dummy,
}

impl Config {
	pub fn usage(&self) -> String {
		let usage = self.usage_impl("", "", "");
		let lines = usage.split("\n")
		                 .map(|s| s.split("\t").collect::<Vec<_>>())
		                 .collect::<Vec<_>>();
		let mut widths = vec![];
		
		for line in &lines {
			if line.len() <= 1 { continue };
			
			for (id, cell) in line.iter().enumerate() {
				if widths.len() <= id {
					widths.push(cell.len());
				} else if widths[id] < cell.len() {
					widths[id] = cell.len();
				}
			}
		}
		
		let mut ret = "Options:\n".to_string();
		
		for line in lines {
			for (id, cell) in line.iter().enumerate() {
				if id + 1 < line.len() {
					ret += &format!("{:<1$}    ", cell, widths[id]);
				} else {
					ret += cell;
				}
			}
			ret += "\n";
		}
		
		ret
	}
	
	pub fn apply_args(&mut self) -> Result<(), ArgsError> {
		let args: Vec<String> = env::args().collect();
		
		let mut opts = Options::new();
		self.prepare_opts(&mut opts, "", "", "")?;
		
		let matches = opts.parse(&args[1..])?;
		self.apply_matches(&matches, "")?;
		
		Ok(())
	}
	
	pub fn default_toml() -> Result<String, toml::ser::Error> {
		toml::to_string_pretty(&Self::default())
	}
}

impl Default for Config {
	fn default() -> Self {
		toml::from_str(include_str!("../config.toml")).expect("Bad config during compilation")
	}
}

args_terminals! { CameraAPI }

impl FromStr for CameraAPI {
	type Err = toml::de::Error;
	fn from_str(s: &str) -> Result<Self, Self::Err> { toml::from_str(&format!("\"{}\"", s)) }
}

impl Display for CameraAPI {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let str = toml::to_string(self).map_err(|_| std::fmt::Error)?;
		f.write_str(&str).map_err(|_| std::fmt::Error)?;
		
		Ok(())
	}
}

lazy_static!(
	static ref CONFIG: ArcSwap<Config> = ArcSwap::default();
);

pub fn get() -> impl Deref<Target = Arc<Config>> + 'static {
	CONFIG.load()
}

pub fn set(config: Config) {
	CONFIG.store(Arc::new(config));
}

pub fn rcu(update: impl Fn(&mut Config)) {
	CONFIG.rcu(|current| {
		let mut new = (**current).clone();
		update(&mut new);
		new
	});
}
