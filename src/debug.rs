use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use std::sync::RwLock;
use std::cell::RefCell;
use std::any::Any;
use cgmath::{Vector3, Vector2, Vector4};

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

pub fn get_flag<T>(key: &str)
                   -> Option<T>
                   where T: Clone + Send + Sync + 'static {
	FLAGS.read()
	     .unwrap()
	     .get(key)
	     .and_then(|val| val.downcast_ref::<T>())
	     .map(|val| val.clone())
}

pub fn get_flag_or_default<T>(key: &str)
                              -> T
                              where T: Clone + Send + Sync + Default + 'static {
	get_flag(key).unwrap_or_default()
}

pub fn set_flag<T>(key: &str, value: T)
	               where T: Clone + Send + Sync + 'static {
	FLAGS.write()
	     .unwrap()
	     .insert(key.to_string(), Box::new(value));
}

pub enum DebugPosition {
	Screen(Vector2<f32>),
	World(Vector3<f32>),
}

impl From<Vector2<f32>> for DebugPosition {
	fn from(vec: Vector2<f32>) -> Self {
		DebugPosition::Screen(vec)
	}
}

impl From<Vector3<f32>> for DebugPosition {
	fn from(vec: Vector3<f32>) -> Self {
		DebugPosition::World(vec)
	}
}

pub struct DebugPoint {
	position: DebugPosition,
	width: f32,
	color: Vector4<f32>
}

pub struct DebugLine {
	from: DebugPosition,
	to: DebugPosition,
	width: f32,
	color: Vector4<f32>
}

thread_local! {
    pub static DEBUG_POINTS: RefCell<Vec<DebugPoint>> = RefCell::new(vec![]);
    pub static DEBUG_LINES: RefCell<Vec<DebugLine>> = RefCell::new(vec![]);
}

pub fn draw_point(position: impl Into<DebugPosition>, width: f32, color: Vector4<f32>) {
	DEBUG_POINTS.with(|points| {
		points.borrow_mut().push(DebugPoint{ position: position.into(), width, color });
	})
}

pub fn draw_line(from: impl Into<DebugPosition>, to: impl Into<DebugPosition>, width: f32, color: Vector4<f32>) {
	DEBUG_LINES.with(|lines| {
		lines.borrow_mut().push(DebugLine{ from: from.into(), to: to.into(), width, color });
	})
}

#[allow(unused_macros)]
macro_rules! dprint {
	($( $args:expr ),*) => { if crate::debug::debug() { print!( $( $args ),* ); } }
}

#[allow(unused_macros)]
macro_rules! dprintln {
	($( $args:expr ),*) => { if crate::debug::debug() { println!( $( $args ),* ); } }
}
