#![feature(bool_to_option)]
#![feature(never_type)]
#![feature(try_blocks)]
#[macro_use] extern crate lazy_static;

use std::error::Error;
use std::env;
use getopts::Options;

#[macro_use] mod debug;
mod renderer;
mod application;
mod utils;

use application::Application;
use application::CameraAPI;
use debug::{set_debug, set_debug_flag};

fn main() -> Result<(), Box<dyn Error>> {
	let args: Vec<String> = env::args().collect();
	let program = args[0].clone();
	let mut opts = Options::new();
	
	opts.optopt("d", "device", "Select fallback device to use", "NUMBER");
	opts.optopt("c", "camera", "Select camera API", "escapi|opencv|openvr|dummy");
	opts.optflag("", "debug", "Enable debugging layer and info");
	opts.optflag("h", "help", "Print this help menu");
	opts.optflag("n", "novr", "Use keyboard and mouse for controls");
	
	let matches = opts.parse(&args[1..])?;
	
	if matches.opt_present("h") {
		print_usage(&program, opts);
		return Ok(());
	}
	
	set_debug(matches.opt_present("debug"));
	set_debug_flag("novr", matches.opt_present("novr"));
	
	let device = matches.opt_get("d")?;
	let camera = matches.opt_get("c")?
	                    .unwrap_or("openvr".to_string())
	                    .to_lowercase();
	
	let camera = match &*camera {
		"opencv" => CameraAPI::OpenCV,
		"openvr" => CameraAPI::OpenVR,
		#[cfg(windows)] "escapi" => CameraAPI::Escapi,
		"dummy" => CameraAPI::Dummy,
		_ => panic!("Unknown camera api: {}", camera),
	};
	
	let application = Application::new(device, camera)?;
	
	application.run()?;
	
	Ok(())
}

fn print_usage(program: &str, opts: Options) {
	let brief = format!("Usage: {} [options]", program);
	print!("{}", opts.usage(&brief));
}
