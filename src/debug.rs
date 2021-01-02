use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use std::sync::RwLock;
use std::any::Any;

static DEBUG: AtomicBool = AtomicBool::new(false);
lazy_static! {
    static ref FLAGS: RwLock<HashMap<String, Box<dyn Any + Send + Sync>>> = RwLock::new(HashMap::new());
}

pub fn debug() -> bool {
	let read = DEBUG.load(Ordering::Relaxed);
	read
}

pub fn set_debug(value: bool) {
	DEBUG.store(value, Ordering::Relaxed);
}

pub fn get_debug_flag<T>(key: &str)
                         -> Option<T>
                         where T: Clone + Send + Sync + 'static {
	FLAGS.read()
	     .unwrap()
	     .get(key)
	     .and_then(|val| val.downcast_ref::<T>())
	     .map(|val| val.clone())
}

pub fn get_debug_flag_or_default<T>(key: &str)
                                   -> T
                                   where T: Clone + Send + Sync + Default + 'static {
	get_debug_flag(key).unwrap_or_default()
}

pub fn set_debug_flag<T>(key: &str, value: T)
	                    where T: Clone + Send + Sync + 'static {
	FLAGS.write()
	     .unwrap()
	     .insert(key.to_string(), Box::new(value));
}

#[allow(unused_macros)]
macro_rules! dprint {
	($( $args:expr ),*) => { if crate::debug::debug() { print!( $( $args ),* ); } }
}

#[allow(unused_macros)]
macro_rules! dprintln {
	($( $args:expr ),*) => { if crate::debug::debug() { println!( $( $args ),* ); } }
}
