use std::ops::Deref;
use std::sync::Arc;
use std::str::FromStr;
use std::fmt::{Display, Formatter};
use std::env;
use serde_derive::{Deserialize, Serialize};
use arc_swap::ArcSwap;
use project39_ar_derive::ConfigPart;
use getopts::{Options, Matches};
use err_derive::Error;

#[derive(Deserialize, Debug, ConfigPart)]
pub struct Config {
	/// Prints this message.
	#[serde(skip)] pub help: bool,
	/// Show debug info.
	pub debug: bool,
	/// Enable validation layers.
	pub validation: bool,
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

#[derive(Deserialize, Debug, ConfigPart)]
pub struct CameraConfig {
	/// Select camera API to use: escapi, opencv, openvr or dummy.
	pub driver: CameraAPI,
	/// Camera device index. Ignored if openvr is used.
	pub id: usize,
}

#[derive(Deserialize, Debug, ConfigPart)]
pub struct NovrConfig {
	/// Enables Non VR mode. The program will not use OpenVR. Use Keyboard and mouse to move.
	pub enabled: bool,
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

trait ConfigPart {
	fn usage_impl(default: Self, path: &str, doc: &str) -> String;
	fn prepare_opts(&mut self, opts: &mut Options, path: &str, doc: &str) -> Result<(), ArgsError>;
	fn apply_matches(&mut self, matches: &mut Matches, path: &str, doc: &str) -> Result<(), ArgsError>;
}

impl Config {
	pub(crate) fn usage() -> String where Self: Default {
		let usage = Self::usage_impl(Self::default(), "", "");
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
	
	pub(crate) fn apply_args(&mut self) -> Result<(), ArgsError> {
		let args: Vec<String> = env::args().collect();
		
		let mut opts = Options::new();
		self.prepare_opts(&mut opts, "", "")?;
		
		let matches = opts.parse(&args[1..])?;
		self.apply_matches(&matches, "", "")?;
		
		Ok(())
	}
}

impl Default for Config {
	fn default() -> Self {
		toml::from_str(include_str!("../config.toml")).expect("Bad config during compilation")
	}
}

impl FromStr for CameraAPI {
	type Err = toml::de::Error;
	
	fn from_str(s: &str) -> Result<Self, Self::Err> { toml::from_str(s) }
}

impl Display for CameraAPI {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let str = toml::to_string(self).map_err(|_| std::fmt::Error)?;
		f.write_str(&str);
		
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

impl ConfigPart for bool {
	fn usage_impl(default: Self, path: &str, doc: &str) -> String {
		if default {
			let (d1, d2) = doc.split_at(1);
			format!("\t--no-{}\tDo not {}{}", path, d1.to_lowercase(), d2)
		} else {
			format!("\t--{}\t{}", path, doc)
		}
	}
	
	fn prepare_opts(&mut self, opts: &mut Options, path: &str, doc: &str) -> Result<(), ArgsError> {
		opts.optopt("", path, doc, &self.to_string());
		Ok(())
	}
	
	fn apply_matches(&mut self, matches: &mut Matches, path: &str, doc: &str) -> Result<(), ArgsError> {
		if let Some(opt) = matches.opt_get(path)? {
			*self = opt;
		}
		Ok(())
	}
}

macro_rules! terminals {
	{ $( $typ:ty )* } => {
		$(
			impl ConfigPart for $typ {
				fn usage_impl(default: Self, path: &str, doc: &str) -> String {
					format!("\t--{} {}\t{}", path, default, doc)
				}
				
				fn prepare_opts(&mut self, opts: &mut Options, path: &str, doc: &str) -> Result<(), ArgsError> {
					opts.optopt("", path, doc, &self.to_string());
					Ok(())
				}
				
				fn apply_matches(&mut self, matches: &mut Matches, path: &str, doc: &str) -> Result<(), ArgsError> {
					if let Some(opt) = matches.opt_get(path)? {
						*self = opt;
					}
					Ok(())
				}
			}
		)*
	}
}

terminals! { f32 f64 u8 i8 u16 i16 u32 i32 u64 i64 usize isize CameraAPI }

#[derive(Debug, Error)]
pub enum ArgsError {
	#[error(display = "{}", _0)] GetoptsError(#[error(source)] getopts::Fail),
}
