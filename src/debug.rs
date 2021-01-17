use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use std::sync::RwLock;
use std::cell::RefCell;
use std::any::Any;
use cgmath::{Vector3, Vector2, Vector4, Matrix4};

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

#[derive(Copy, Clone)]
pub enum DebugPosition {
	Screen(Vector2<f32>),
	World(Vector3<f32>),
}

impl DebugPosition {
	pub fn project(self, viewproj: Matrix4<f32>) -> Vector3<f32> {
		match self {
			DebugPosition::Screen(screen) => screen.extend(0.0),
			DebugPosition::World(world) => {
				let screen = viewproj * world.extend(1.0);
				return screen.truncate() / screen.w;
			},
		}
	}
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
	pub position: DebugPosition,
	pub radius: f32,
	pub color: Vector4<f32>
}

pub struct DebugLine {
	pub from: DebugPosition,
	pub to: DebugPosition,
	pub width: f32,
	pub color: Vector4<f32>
}

#[derive(Copy, Clone)]
pub enum DebugOffset {
	Center(Vector2<f32>),
	TopLeft(Vector2<f32>),
	Top(Vector2<f32>),
	TopRight(Vector2<f32>),
	Right(Vector2<f32>),
	BottomRight(Vector2<f32>),
	Bottom(Vector2<f32>),
	BottomLeft(Vector2<f32>),
	Left(Vector2<f32>),
}

impl DebugOffset {
	pub fn center(x: f32, y: f32)       -> DebugOffset { DebugOffset::Center(Vector2::new(x, y)) }
	pub fn top_left(x: f32, y: f32)     -> DebugOffset { DebugOffset::TopLeft(Vector2::new(x, y)) }
	pub fn top(x: f32, y: f32)          -> DebugOffset { DebugOffset::Top(Vector2::new(x, y)) }
	pub fn top_right(x: f32, y: f32)    -> DebugOffset { DebugOffset::TopRight(Vector2::new(x, y)) }
	pub fn right(x: f32, y: f32)        -> DebugOffset { DebugOffset::Right(Vector2::new(x, y)) }
	pub fn bottom_right(x: f32, y: f32) -> DebugOffset { DebugOffset::BottomRight(Vector2::new(x, y)) }
	pub fn bottom(x: f32, y: f32)       -> DebugOffset { DebugOffset::Bottom(Vector2::new(x, y)) }
	pub fn bottom_left(x: f32, y: f32)  -> DebugOffset { DebugOffset::BottomLeft(Vector2::new(x, y)) }
	pub fn left(x: f32, y: f32)         -> DebugOffset { DebugOffset::Left(Vector2::new(x, y)) }
}

impl DebugOffset {
	pub fn evaluate(&self, size: Vector2<f32>) -> Vector2<f32> {
		match self {
			DebugOffset::Center(offset)      => offset - Vector2::new( size.x * 0.5, size.y * 0.5),
			DebugOffset::TopLeft(offset)     => offset - Vector2::new( size.x * 1.0, size.y * 1.0),
			DebugOffset::Top(offset)         => offset - Vector2::new( size.x * 0.5, size.y * 1.0),
			DebugOffset::TopRight(offset)    => offset - Vector2::new(          0.0, size.y * 1.0),
			DebugOffset::Right(offset)       => offset - Vector2::new(          0.0, size.y * 0.5),
			DebugOffset::BottomRight(offset) => offset - Vector2::new(          0.0,          0.0),
			DebugOffset::Bottom(offset)      => offset - Vector2::new( size.x * 0.5,          0.0),
			DebugOffset::BottomLeft(offset)  => offset - Vector2::new( size.x * 1.0,          0.0),
			DebugOffset::Left(offset)        => offset - Vector2::new( size.x * 1.0, size.y * 0.5),
		}
	}
}

pub struct DebugText {
	pub text: String,
	pub position: DebugPosition,
	pub offset: DebugOffset,
	pub size: f32,
	pub color: Vector4<f32>
}

thread_local! {
    pub static DEBUG_POINTS: RefCell<Vec<DebugPoint>> = RefCell::new(vec![]);
    pub static DEBUG_LINES: RefCell<Vec<DebugLine>> = RefCell::new(vec![]);
    pub static DEBUG_TEXTS: RefCell<Vec<DebugText>> = RefCell::new(vec![]);
}

pub fn draw_point(position: impl Into<DebugPosition>, radius: f32, color: Vector4<f32>) {
	DEBUG_POINTS.with(|points| {
		points.borrow_mut().push(DebugPoint{ position: position.into(), radius, color });
	})
}

pub fn draw_line(from: impl Into<DebugPosition>, to: impl Into<DebugPosition>, width: f32, color: Vector4<f32>) {
	DEBUG_LINES.with(|lines| {
		lines.borrow_mut().push(DebugLine{ from: from.into(), to: to.into(), width, color });
	})
}

pub fn draw_text(text: impl Into<String>, position: impl Into<DebugPosition>, offset: DebugOffset, size: f32, color: Vector4<f32>) {
	DEBUG_TEXTS.with(|texts| {
		texts.borrow_mut().push(DebugText{ text: text.into(), position: position.into(), offset, size, color });
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
