#![feature(bool_to_option)]
#![feature(never_type)]
#![feature(try_blocks)]
#![feature(trace_macros)]
#![feature(type_name_of_val)]
#![feature(backtrace)]
#![feature(osstring_ascii)]
#![feature(vec_into_raw_parts)]
#[macro_use] extern crate lazy_static;

use std::env;
use std::panic;
use std::fmt::Debug;
use std::panic::PanicInfo;
use std::error::Error;
use std::time::Duration;
use getopts::Options;
use err_derive::Error;
use native_dialog::{MessageDialog, MessageType};

#[macro_use] #[allow(dead_code)] mod debug;
#[allow(dead_code)] mod math;
mod renderer;
mod application;
mod utils;

use application::{Application, ApplicationCreationError, ApplicationRunError};


fn main() {
	panic::set_hook(Box::new(panic_hook()));
	
	let result = run_application();
	
	if let Err(err) = result {
		let message = format!("{}\n\nError {:?}", err.to_string(), err);
		
		eprintln!("{}", message);
		
		if let Some(backtrace) = err.backtrace() {
			eprintln!("{}", backtrace);
		}
		
		MessageDialog::new()
		              .set_type(MessageType::Error)
		              .set_title(&err.to_string())
		              .set_text(&message)
		              .show_alert()
		              .unwrap();
	}
}

fn run_application() -> Result<(), RunError> {
	let args: Vec<String> = env::args().collect();
	let program = args[0].clone();
	let mut opts = Options::new();
	
	opts.optopt("d", "device", "Select fallback device to use", "NUMBER");
	opts.optopt("n", "models", "Comma-separated models counts, default: 1,5,10,20,50,100", "COUNTS");
	opts.optopt("m", "morphs", "Comma-separated morphs counts, default: 0,1,2,4,8,16,32,64", "COUNTS");
	opts.optopt("", "spindur", "Warm up duration in seconds, default: 1", "TIME");
	opts.optopt("t", "testdur", "Test duration in seconds, default: 10", "TIME");
	opts.optopt("s", "seed", "RNG seed, 64-bit unsigned integer", "NUMBER");
	opts.optflag("", "debug", "Enable debugging layer and info");
	opts.optflag("h", "help", "Print this help menu");
	
	let matches = opts.parse(&args[1..])?;
	
	if matches.opt_present("h") {
		print_usage(&program, opts);
		return Ok(());
	}
	
	debug::set_debug(matches.opt_present("debug"));
	
	let device = matches.opt_get("device")?;
	let models: Vec<usize> = matches.opt_get("models")?
	                                .unwrap_or_else(|| "1,5,10,20,50,100".to_string())
	                                .split(",")
	                                .map(str::parse)
	                                .collect::<Result<_, _>>()?;
	let morphs: Vec<usize> = matches.opt_get("morphs")?
	                                .unwrap_or_else(|| "0,1,2,4,8,16,32,64".to_string())
	                                .split(",")
	                                .map(str::parse)
	                                .collect::<Result<_, _>>()?;
	let spin_dur = Duration::from_secs_f32(matches.opt_get("spindur")?.unwrap_or(1.0));
	let test_dur = Duration::from_secs_f32(matches.opt_get("testdur")?.unwrap_or(10.0));
	let seed = matches.opt_get("seed")?
	                  .map(|s: String| s.parse())
	                  .transpose()?
	                  .unwrap_or(rand::random());
	
	let application = Application::new(device, seed)?;
	
	application.run(&models, &morphs, spin_dur, test_dur)?;
	
	Ok(())
}

fn print_usage(program: &str, opts: Options) {
	let brief = format!("Usage: {} [options]", program);
	print!("{}", opts.usage(&brief));
}

fn panic_hook() -> impl Fn(&PanicInfo) {
	let default_hook = panic::take_hook();
	
	move |info| {
		default_hook(info);
		
		let payload;
		if let Some(string) = info.payload().downcast_ref::<String>() {
			payload = string.clone()
		} else if let Some(string) = info.payload().downcast_ref::<&'static str>() {
			payload = string.to_string()
		} else {
			payload = format!("Unformattable panic payload! ({})", std::any::type_name_of_val(info.payload()))
		};
		
		let thread = std::thread::current()
		                         .name()
		                         .unwrap_or("<unnamed>")
		                         .to_string();
		
		let location;
		if let Some(loc) = info.location() {
			location = loc.to_string();
		} else {
			location = "Unknown Location".to_string();
		}
		
		let message = format!("{}\n\nThread {} panicked at {}", payload, thread, location);
		
		MessageDialog::new()
		              .set_type(MessageType::Error)
		              .set_title("Fatal Error")
		              .set_text(&message)
		              .show_alert()
		              .unwrap();
	}
}

#[derive(Debug, Error)]
pub enum RunError {
	#[error(display = "{}", _0)] ApplicationCreationError(#[error(source)] ApplicationCreationError),
	#[error(display = "{}", _0)] ApplicationRunError(#[error(source)] ApplicationRunError),
	#[error(display = "{}", _0)] GetoptsError(#[error(source)] getopts::Fail),
	#[error(display = "{}", _0)] ParseIntError(#[error(source)] std::num::ParseIntError),
	#[error(display = "{}", _0)] ParseFloatError(#[error(source)] std::num::ParseFloatError),
	#[error(display = "{}", _0)] Infallible(#[error(source)] std::convert::Infallible),
}
