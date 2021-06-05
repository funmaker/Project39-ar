use std::ops::Deref;
use std::sync::Arc;
use std::str::FromStr;
use std::fmt::{Display, Formatter};
use std::env;
use serde_derive::{Deserialize, Serialize};
use arc_swap::ArcSwap;
use project39_ar_derive::FromArgs;
use getopts::{Options, Matches};
use err_derive::Error;
use std::error::Error;

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
	/// Select camera API to use: escapi, opencv, openvr or dummy.
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

trait FromArgs {
	fn usage_impl(&self, short: &str, path: &str, doc: &str) -> String;
	fn prepare_opts(&mut self, opts: &mut Options, short: &str, path: &str, doc: &str) -> Result<(), ArgsError>;
	fn apply_matches(&mut self, matches: &Matches, path: &str) -> Result<(), ArgsError>;
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

impl FromArgs for bool {
	fn usage_impl(&self, short: &str, path: &str, doc: &str) -> String {
		let short_flag = if short.len() > 0 {
			format!("-{}, ", short)
		} else {
			"    ".to_string()
		};
		
		if *self {
			let (d1, d2) = doc.split_at(1);
			format!("\t{}--no-{}\tDo not {}{}", short_flag.to_uppercase(), path, d1.to_lowercase(), d2)
		} else {
			format!("\t{}--{}\t{}", short_flag, path, doc)
		}
	}
	
	fn prepare_opts(&mut self, opts: &mut Options, short: &str, path: &str, doc: &str) -> Result<(), ArgsError> {
		let neg = format!("no-{}", path);
		let short_neg = short.to_uppercase();
		opts.optflag(short, path, doc);
		opts.optflag(&short_neg, &neg, doc);
		Ok(())
	}
	
	fn apply_matches(&mut self, matches: &Matches, path: &str) -> Result<(), ArgsError> {
		let neg = format!("no-{}", path);
		if matches.opt_present(path) { *self = true; }
		if matches.opt_present(&neg) { *self = false; }
		Ok(())
	}
}

macro_rules! terminals {
	{ $( $typ:ty )* } => {
		$(
			impl FromArgs for $typ {
				fn usage_impl(&self, short: &str, path: &str, doc: &str) -> String {
					let short_flag = if short.len() > 0 {
						format!("-{}, ", short)
					} else {
						"    ".to_string()
					};
					
					format!("\t{}--{} {}\t{}", short_flag, path, self, doc)
				}
				
				fn prepare_opts(&mut self, opts: &mut Options, short: &str, path: &str, doc: &str) -> Result<(), ArgsError> {
					opts.optopt(short, path, doc, &self.to_string());
					Ok(())
				}
				
				fn apply_matches(&mut self, matches: &Matches, path: &str) -> Result<(), ArgsError> {
					if let Some(str) = matches.opt_str(path) {
						*self = str.parse::<Self>()
						           .map_err(|err| ArgsError::BadArgument(path.to_string(), err.into()))?;
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
	#[error(display = "Failed to parse cli argument {}: {}", _0, _1)] BadArgument(String, Box<dyn Error>),
}
