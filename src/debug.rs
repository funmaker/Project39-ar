use std::sync::atomic::{AtomicBool, Ordering};

const DEBUG: AtomicBool = AtomicBool::new(false);

pub fn debug() -> bool {
	DEBUG.load(Ordering::Relaxed)
}

pub fn set_debug(value: bool) {
	DEBUG.store(value, Ordering::Relaxed);
}

#[allow(unused_macros)]
macro_rules! dprint {
	($( $args:expr ),*) => { if crate::debug::debug() { print!( $( $args ),* ); } }
}

#[allow(unused_macros)]
macro_rules! dprintln {
	($( $args:expr ),*) => { if crate::debug::debug() { println!( $( $args ),* ); } }
}
