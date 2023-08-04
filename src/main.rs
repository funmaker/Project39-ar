#![feature(never_type)]
#![feature(try_blocks)]
#![feature(trace_macros)]
#![feature(type_name_of_val)]
#![feature(vec_into_raw_parts)]
#![feature(negative_impls)]
#![feature(extract_if)]
#![feature(hash_extract_if)]
#![feature(path_file_prefix)]
#![feature(array_chunks)]
#![feature(int_roundings)]

#[macro_use] extern crate lazy_static;
#[macro_use] extern crate nalgebra;
extern crate core;

use std::{fs, panic};
use std::fmt::Debug;
use std::panic::PanicInfo;
use err_derive::Error;
use native_dialog::{MessageDialog, MessageType};

#[macro_use] #[allow(dead_code)] mod debug;
#[macro_use] mod utils;
mod application;
#[allow(dead_code)] mod component;
mod config;
#[allow(dead_code)] mod math;
mod renderer;

use application::{Application, ApplicationCreationError, ApplicationRunError};
use config::{Config, Color};
use utils::from_args::ArgsError;


fn main() {
	panic::set_hook(Box::new(panic_hook()));
	
	let result = run_application();
	
	if let Err(err) = result {
		let message = format!("{}\n\nError {:?}", err.to_string(), err);
		
		eprintln!("{}", message);
		
		MessageDialog::new()
		              .set_type(MessageType::Error)
		              .set_title(&err.to_string())
		              .set_text(&message)
		              .show_alert()
		              .unwrap();
	}
}

fn run_application() -> Result<(), RunError> {
	let config_path = "config.toml";
	let file_name = std::env::args().next().unwrap_or("project39-ar.exe".to_string());
	
	let mut config = if fs::metadata(config_path).is_ok() {
		let config_file = fs::read_to_string(config_path)?;
		toml::from_str(&config_file)?
	} else {
		eprintln!("\nUnable to locate config.toml!");
		eprintln!("Use `{} --example_config` to print an example config.\n", file_name);
		
		Config::default()
	};
	
	if let Err(err) = config.apply_args() {
		print_usage(&file_name, config);
		return Err(err.into());
	}
	
	if config.help {
		print_usage(&file_name, config);
		return Ok(());
	}
	
	if config.example_config {
		println!("{}", Config::default_toml()?);
		return Ok(());
	}
	
	match config.color {
		Color::Auto => colored::control::unset_override(),
		Color::Never => colored::control::set_override(false),
		Color::Always => colored::control::set_override(true),
	}
	
	debug::set_debug(config.debug);
	config::set(config);
	
	let application = Application::new()?;
	application.run()?;
	
	Ok(())
}

fn print_usage(filename: &str, mut config: Config) {
	config.help = false;
	
	println!("Usage:");
	println!("    {} [options]", filename);
	println!("\n{}", config.usage());
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
	#[error(display = "{}", _0)] ArgsError(#[error(source)] ArgsError),
	#[error(display = "{}", _0)] IOError(#[error(source)] std::io::Error),
	#[error(display = "{}", _0)] DeserializationError(#[error(source)] toml::de::Error),
	#[error(display = "{}", _0)] SerializationError(#[error(source)] toml::ser::Error),
}
