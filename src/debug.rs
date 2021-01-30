use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use std::sync::RwLock;
use std::cell::RefCell;
use std::any::Any;

use crate::math::{Point2, Point3, Color, Vec2, Translation2, PMat4};

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

#[allow(unused_macros)]
macro_rules! dprint {
	($( $args:expr ),*) => { if crate::debug::debug() { print!( $( $args ),* ); } }
}

#[allow(unused_macros)]
macro_rules! dprintln {
	($( $args:expr ),*) => { if crate::debug::debug() { println!( $( $args ),* ); } }
}

#[derive(Copy, Clone)]
pub enum DebugPosition {
	Screen(Point2),
	World(Point3),
}

impl DebugPosition {
	pub fn project(self, viewproj: &PMat4) -> Point3 {
		match self {
			DebugPosition::Screen(screen) => screen.coords.push(0.0).into(),
			DebugPosition::World(world) => viewproj.transform_point(&world),
		}
	}
}

impl From<Point2> for DebugPosition {
	fn from(pos: Point2) -> Self {
		DebugPosition::Screen(pos)
	}
}

impl From<&Point2> for DebugPosition {
	fn from(pos: &Point2) -> Self {
		DebugPosition::Screen(pos.clone())
	}
}

impl From<Point3> for DebugPosition {
	fn from(pos: Point3) -> Self {
		DebugPosition::World(pos)
	}
}

impl From<&Point3> for DebugPosition {
	fn from(pos: &Point3) -> Self {
		DebugPosition::World(pos.clone())
	}
}

pub struct DebugPoint {
	pub position: DebugPosition,
	pub radius: f32,
	pub color: Color,
}

pub struct DebugLine {
	pub from: DebugPosition,
	pub to: DebugPosition,
	pub width: f32,
	pub color: Color,
}

#[derive(Clone)]
pub enum DebugOffset {
	Center(Translation2),
	TopLeft(Translation2),
	Top(Translation2),
	TopRight(Translation2),
	Right(Translation2),
	BottomRight(Translation2),
	Bottom(Translation2),
	BottomLeft(Translation2),
	Left(Translation2),
}

impl DebugOffset {
	pub fn top_left(x: f32, y: f32)     -> DebugOffset { DebugOffset::TopLeft(Translation2::new(x, y)) }
	pub fn top(x: f32, y: f32)          -> DebugOffset { DebugOffset::Top(Translation2::new(x, y)) }
	pub fn top_right(x: f32, y: f32)    -> DebugOffset { DebugOffset::TopRight(Translation2::new(x, y)) }
	pub fn left(x: f32, y: f32)         -> DebugOffset { DebugOffset::Left(Translation2::new(x, y)) }
	pub fn center(x: f32, y: f32)       -> DebugOffset { DebugOffset::Center(Translation2::new(x, y)) }
	pub fn right(x: f32, y: f32)        -> DebugOffset { DebugOffset::Right(Translation2::new(x, y)) }
	pub fn bottom_left(x: f32, y: f32)  -> DebugOffset { DebugOffset::BottomLeft(Translation2::new(x, y)) }
	pub fn bottom(x: f32, y: f32)       -> DebugOffset { DebugOffset::Bottom(Translation2::new(x, y)) }
	pub fn bottom_right(x: f32, y: f32) -> DebugOffset { DebugOffset::BottomRight(Translation2::new(x, y)) }
}

impl DebugOffset {
	pub fn evaluate(&self, size: Vec2) -> Point2 {
		match self {
			DebugOffset::TopLeft(offset)     => offset * Point2::new(-size.x * 1.0, -size.y * 1.0),
			DebugOffset::Top(offset)         => offset * Point2::new(-size.x * 0.5, -size.y * 1.0),
			DebugOffset::TopRight(offset)    => offset * Point2::new(          0.0, -size.y * 1.0),
			DebugOffset::Left(offset)        => offset * Point2::new(-size.x * 1.0, -size.y * 0.5),
			DebugOffset::Center(offset)      => offset * Point2::new(-size.x * 0.5, -size.y * 0.5),
			DebugOffset::Right(offset)       => offset * Point2::new(          0.0, -size.y * 0.5),
			DebugOffset::BottomLeft(offset)  => offset * Point2::new(-size.x * 1.0,           0.0),
			DebugOffset::Bottom(offset)      => offset * Point2::new(-size.x * 0.5,           0.0),
			DebugOffset::BottomRight(offset) => offset * Point2::new(          0.0,           0.0),
		}
	}
}

pub struct DebugText {
	pub text: String,
	pub position: DebugPosition,
	pub offset: DebugOffset,
	pub size: f32,
	pub color: Color,
}

thread_local! {
    pub static DEBUG_POINTS: RefCell<Vec<DebugPoint>> = RefCell::new(vec![]);
    pub static DEBUG_LINES: RefCell<Vec<DebugLine>> = RefCell::new(vec![]);
    pub static DEBUG_TEXTS: RefCell<Vec<DebugText>> = RefCell::new(vec![]);
}

pub fn draw_point(position: impl Into<DebugPosition>, radius: f32, color: Color) {
	DEBUG_POINTS.with(|points| {
		points.borrow_mut().push(DebugPoint{ position: position.into(), radius, color });
	})
}

pub fn draw_line(from: impl Into<DebugPosition>, to: impl Into<DebugPosition>, width: f32, color: Color) {
	DEBUG_LINES.with(|lines| {
		lines.borrow_mut().push(DebugLine{ from: from.into(), to: to.into(), width, color });
	})
}

pub fn draw_text(text: impl Into<String>, position: impl Into<DebugPosition>, offset: DebugOffset, size: f32, color: Color) {
	DEBUG_TEXTS.with(|texts| {
		texts.borrow_mut().push(DebugText{ text: text.into(), position: position.into(), offset, size, color });
	})
}

fn split_comma(line: &str) -> Option<(&str, &str)> {
	let mut splitter = line.splitn(2, ',');
	let first = splitter.next()?;
	let second = splitter.next()?;
	Some((first, second))
}

lazy_static! {
	static ref TRANSLATIONS: HashMap<&'static str, &'static str> = include_str!("./translations.txt").lines().filter_map(split_comma).collect();
}

pub fn translate(text: &str) -> Option<&'static str> {
	TRANSLATIONS.get(text)
	            .cloned() // Remove double ref
}
