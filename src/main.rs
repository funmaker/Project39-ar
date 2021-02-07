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
use getopts::Options;
use err_derive::Error;
use native_dialog::{MessageDialog, MessageType};

#[macro_use] #[allow(dead_code)] mod debug;
#[allow(dead_code)] mod math;
mod renderer;
mod application;
mod utils;

use application::{Application, CameraAPI, ApplicationCreationError, ApplicationRunError};


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
	opts.optopt("c", "camera", "Select camera API", "escapi|opencv|openvr|dummy");
	opts.optflag("", "debug", "Enable debugging layer and info");
	opts.optflag("h", "help", "Print this help menu");
	opts.optflag("n", "novr", "Non VR mode. The program will not use OpenVR. Use Keyboard and mouse to move.");
	
	let matches = opts.parse(&args[1..])?;
	
	if matches.opt_present("h") {
		print_usage(&program, opts);
		return Ok(());
	}
	
	debug::set_debug(matches.opt_present("debug"));
	
	let device = matches.opt_get("d")?;
	let camera = matches.opt_get("c")?
	                    .unwrap_or("openvr".to_string())
	                    .to_lowercase();
	let novr = matches.opt_present("novr");
	
	let camera = match &*camera {
		"opencv" => CameraAPI::OpenCV,
		"openvr" => CameraAPI::OpenVR,
		#[cfg(windows)] "escapi" => CameraAPI::Escapi,
		"dummy" => CameraAPI::Dummy,
		_ => return Err(RunError::BadCamera(camera)),
	};
	
	let application = Application::new(device, camera, !novr)?;
	
	application.run()?;
	
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
	#[error(display = "Unknown camera provider: {}", _0)] BadCamera(String),
	#[error(display = "{}", _0)] ApplicationCreationError(#[error(source)] ApplicationCreationError),
	#[error(display = "{}", _0)] ApplicationRunError(#[error(source)] ApplicationRunError),
	#[error(display = "{}", _0)] GetoptsError(#[error(source)] getopts::Fail),
	#[error(display = "{}", _0)] ParseIntError(#[error(source)] std::num::ParseIntError),
	#[error(display = "{}", _0)] Infallible(#[error(source)] std::convert::Infallible),
}
