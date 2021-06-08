use std::ops::Deref;
use std::sync::Arc;
use std::str::FromStr;
use std::fmt::{Display, Formatter};
use std::env;
use serde_derive::{Deserialize, Serialize};
use arc_swap::ArcSwap;
use getopts::{Options, Matches};
use crate::utils::from_args::{FromArgs, ArgsError, args_terminals};

#[derive(Deserialize, Serialize, Debug, FromArgs)]
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
	/// Multi-Sampling Anti-Aliasing factor.
	pub ssaa: f32,
	/// Super-Sampling Anti-Aliasing factor.
	pub msaa: u32,
	/// Camera Configuration
	pub camera: CameraConfig,
	/// Non VR mode
	pub novr: NovrConfig,
}

#[derive(Deserialize, Serialize, Debug, FromArgs)]
pub struct CameraConfig {
	/// Select background API to use: escapi, opencv, openvr or dummy.
	#[arg_short = "c"] #[arg_rename = ""] pub driver: CameraAPI,
	/// Camera device index. Ignored if openvr is used.
	pub id: usize,
}

#[derive(Deserialize, Serialize, Debug, FromArgs)]
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

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CameraAPI {
	#[cfg(windows)] Escapi,
	OpenCV,
	OpenVR,
	Dummy,
}

args_terminals! { CameraAPI }

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
