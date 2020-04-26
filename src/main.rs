use std::error::Error;
use std::env;
use getopts::Options;

#[macro_use] mod debug;
mod shaders;
mod renderer;
mod application;
mod openvr_vulkan;

use application::Application;
use crate::debug::set_debug;

fn main() -> Result<(), Box<dyn Error>> {
	let args: Vec<String> = env::args().collect();
	let program = args[0].clone();
	let mut opts = Options::new();
	
	opts.optopt("d", "device", "Select fallback device to use", "NUMBER");
	opts.optflag("", "debug", "Enable debugging layer and info");
	opts.optflag("h", "help", "Print this help menu");
	
	let matches = opts.parse(&args[1..])?;
	
	if matches.opt_present("h") {
		print_usage(&program, opts);
		return Ok(());
	}
	
	set_debug(matches.opt_present("debug"));
	
	let device = matches.opt_get("d")?;
	
	let application = Application::new(device)?;
	
	application.run()?;
	
	Ok(())
}

fn print_usage(program: &str, opts: Options) {
	let brief = format!("Usage: {} [options]", program);
	print!("{}", opts.usage(&brief));
}
