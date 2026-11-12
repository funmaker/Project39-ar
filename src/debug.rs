use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::math::{Point2, Point3, Color, Vec2, Translation2, PMat4, Isometry3, Vec3, Similarity3, Translation3};


/// Global debug mode toggle (`--debug`).
static DEBUG: AtomicBool = AtomicBool::new(false);
lazy_static! {
    static ref FLAGS: RwLock<HashMap<String, Box<dyn Any + Send + Sync>>> = RwLock::new(HashMap::new());
}

/// Returns `true` if debug mode is enabled.
pub fn debug() -> bool {
	let read = DEBUG.load(Ordering::Relaxed);
	read
}

/// No-op function to put a breakpoint on.
pub fn debugger() {
	() // Breakpoint
}

pub fn set_debug(value: bool) {
	DEBUG.store(value, Ordering::Relaxed);
}

/// Returns the value of a debug flag if it is set and has type `T`.
///
/// Debug flags are global, string-keyed values used for debug toggles and for passing
/// debug values around (eg. `"DebugCollidersDraw"`).
pub fn get_flag<T>(key: &str)
                   -> Option<T>
                   where T: Clone + Send + Sync + 'static {
	FLAGS.read()
	     .unwrap()
	     .get(key)
	     .and_then(|val| val.downcast_ref::<T>())
	     .map(|val| val.clone())
}

/// Returns the value of a debug flag, or `T::default()` if it's not set or has a different type.
pub fn get_flag_or_default<T>(key: &str)
                              -> T
                              where T: Clone + Send + Sync + Default + 'static {
	get_flag(key).unwrap_or_default()
}

/// Sets a debug flag.
pub fn set_flag<T>(key: &str, value: T)
	               where T: Clone + Send + Sync + 'static {
	FLAGS.write()
	     .unwrap()
	     .insert(key.to_string(), Box::new(value));
}

/// Sets the default values of debug flags that don't default to `T::default()`.
pub fn setup_default_flags() {
	set_flag("DebugGizmoSize", 1.0_f32);
	set_flag("DebugGizmoFallOff", 1.0_f32);
}

/// `print!` that only prints in debug mode.
#[allow(unused_macros)]
macro_rules! dprint {
	($( $args:expr ),*) => { if crate::debug::debug() { print!( $( $args ),* ); } }
}

/// `println!` that only prints in debug mode.
#[allow(unused_macros)]
macro_rules! dprintln {
	($( $args:expr ),*) => { if crate::debug::debug() { println!( $( $args ),* ); } }
}

/// Position of a debug primitive: in screen space (normalized device coordinates) or in world space.
#[derive(Copy, Clone)]
pub enum DebugPosition {
	Screen(Point2),
	World(Point3),
}

impl DebugPosition {
	/// Projects the position into clip space of both eyes using their view-projection matrices.
	pub fn project(self, viewproj: &(PMat4, PMat4)) -> (Point3, Point3) {
		match self {
			DebugPosition::Screen(screen) => (screen.coords.push(0.0).into(), screen.coords.push(0.0).into()),
			DebugPosition::World(world) => (viewproj.0.transform_point(&world), viewproj.1.transform_point(&world)),
		}
	}
}

impl From<Point2> for DebugPosition {
	fn from(pos: Point2) -> Self {
		DebugPosition::Screen(pos)
	}
}

impl From<Point3> for DebugPosition {
	fn from(pos: Point3) -> Self {
		DebugPosition::World(pos)
	}
}

impl From<Vec3> for DebugPosition {
	fn from(pos: Vec3) -> Self {
		DebugPosition::World(pos.into())
	}
}

impl From<Translation3> for DebugPosition {
	fn from(pos: Translation3) -> Self {
		DebugPosition::World(pos * Point3::origin())
	}
}

impl From<Isometry3> for DebugPosition {
	fn from(pos: Isometry3) -> Self {
		DebugPosition::World(pos * Point3::origin())
	}
}

impl From<Similarity3> for DebugPosition {
	fn from(pos: Similarity3) -> Self {
		DebugPosition::World(pos * Point3::origin())
	}
}

impl From<glamx::Vec2> for DebugPosition {
	fn from(pos: glamx::Vec2) -> Self {
		DebugPosition::Screen(pos.into())
	}
}

impl From<glamx::Vec3> for DebugPosition {
	fn from(pos: glamx::Vec3) -> Self {
		DebugPosition::World(pos.into())
	}
}

impl From<glamx::Pose2> for DebugPosition {
	fn from(pos: glamx::Pose2) -> Self {
		DebugPosition::Screen(pos.translation.into())
	}
}

impl From<glamx::Pose3> for DebugPosition {
	fn from(pos: glamx::Pose3) -> Self {
		DebugPosition::World(pos.translation.into())
	}
}

/// Debug point, `radius` is in pixels.
pub struct DebugPoint {
	pub position: DebugPosition,
	pub radius: f32,
	pub color: Color,
}

/// Debug line, `width` is in pixels.
pub struct DebugLine {
	pub from: DebugPosition,
	pub to: DebugPosition,
	pub width: f32,
	pub color: Color,
}

/// Anchor of a debug text relative to its position, with an additional offset in pixels.
///
/// eg. `BottomRight` places the text below and to the right of the position.
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
	/// Returns the top left corner of a text of the given size, relative to its position.
	pub fn evaluate(&self, size: Vec2) -> Point2 {
		match self {
			DebugOffset::TopLeft(offset)     => offset * point!(-size.x * 1.0, -size.y * 1.0),
			DebugOffset::Top(offset)         => offset * point!(-size.x * 0.5, -size.y * 1.0),
			DebugOffset::TopRight(offset)    => offset * point!(          0.0, -size.y * 1.0),
			DebugOffset::Left(offset)        => offset * point!(-size.x * 1.0, -size.y * 0.5),
			DebugOffset::Center(offset)      => offset * point!(-size.x * 0.5, -size.y * 0.5),
			DebugOffset::Right(offset)       => offset * point!(          0.0, -size.y * 0.5),
			DebugOffset::BottomLeft(offset)  => offset * point!(-size.x * 1.0,           0.0),
			DebugOffset::Bottom(offset)      => offset * point!(-size.x * 0.5,           0.0),
			DebugOffset::BottomRight(offset) => offset * point!(          0.0,           0.0),
		}
	}
}

/// Debug text label, `size` is the font size in pixels.
pub struct DebugText {
	pub text: String,
	pub position: DebugPosition,
	pub offset: DebugOffset,
	pub size: f32,
	pub color: Color,
}

/// Debug box with filled faces and outlined edges.
pub struct DebugBox {
	pub position: Isometry3,
	pub size: Vec3,
	pub color: Color,
	pub edge: Color,
}

/// Debug sphere with filled faces and outlined edges.
pub struct DebugSphere {
	pub position: Isometry3,
	pub radius: f32,
	pub color: Color,
	pub edge: Color,
}

/// Debug capsule between two points with filled faces and outlined edges.
pub struct DebugCapsule {
	pub point_a: Point3,
	pub point_b: Point3,
	pub radius: f32,
	pub color: Color,
	pub edge: Color,
}

// Debug primitives queued for drawing this frame. Drawn and cleared by the `DebugRenderer`.
thread_local! {
    pub static DEBUG_POINTS: RefCell<Vec<DebugPoint>> = RefCell::new(vec![]);
    pub static DEBUG_LINES: RefCell<Vec<DebugLine>> = RefCell::new(vec![]);
    pub static DEBUG_TEXTS: RefCell<Vec<DebugText>> = RefCell::new(vec![]);
    pub static DEBUG_BOXES: RefCell<Vec<DebugBox>> = RefCell::new(vec![]);
    pub static DEBUG_SPHERES: RefCell<Vec<DebugSphere>> = RefCell::new(vec![]);
    pub static DEBUG_CAPSULES: RefCell<Vec<DebugCapsule>> = RefCell::new(vec![]);
}

/// Queues a debug point to be drawn this frame.
pub fn draw_point(position: impl Into<DebugPosition>, radius: f32, color: Color) {
	DEBUG_POINTS.with(|points| {
		points.borrow_mut().push(DebugPoint{ position: position.into(), radius, color });
	})
}

/// Queues a debug line to be drawn this frame.
pub fn draw_line(from: impl Into<DebugPosition>, to: impl Into<DebugPosition>, width: f32, color: Color) {
	DEBUG_LINES.with(|lines| {
		lines.borrow_mut().push(DebugLine{ from: from.into(), to: to.into(), width, color });
	})
}

/// Queues a debug text to be drawn this frame.
pub fn draw_text(text: impl Into<String>, position: impl Into<DebugPosition>, offset: DebugOffset, size: f32, color: Color) {
	DEBUG_TEXTS.with(|texts| {
		texts.borrow_mut().push(DebugText{ text: text.into(), position: position.into(), offset, size, color });
	})
}

/// Queues a debug box to be drawn this frame.
pub fn draw_box(position: Isometry3, size: Vec3, color: Color, edge: Color) {
	DEBUG_BOXES.with(|boxes| {
		boxes.borrow_mut().push(DebugBox{ position, size, color, edge });
	})
}

/// Queues a debug sphere to be drawn this frame.
pub fn draw_sphere(position: Isometry3, radius: f32, color: Color, edge: Color) {
	DEBUG_SPHERES.with(|spheres| {
		spheres.borrow_mut().push(DebugSphere{ position, radius, color, edge });
	})
}

/// Queues a debug capsule to be drawn this frame.
pub fn draw_capsule(point_a: Point3, point_b: Point3, radius: f32, color: Color, edge: Color) {
	DEBUG_CAPSULES.with(|capsules| {
		capsules.borrow_mut().push(DebugCapsule{ point_a, point_b, radius, color, edge });
	})
}

/// Returns the scale of a debug gizmo at the given position, based on the `DebugGizmoSize` and
/// `DebugGizmoFallOff` flags and the distance from `DebugGizmoPoV`.
///
/// With a fall off of 1, gizmos keep the same world size. With 0, they keep the same screen size.
pub fn gizmo_scale(pos: impl Into<DebugPosition>) -> f32 {
	let pos = pos.into();
	let gizmo_size = get_flag("DebugGizmoSize").unwrap_or(1.0_f32);
	let gizmo_fallout = get_flag("DebugGizmoFallOff").unwrap_or(1.0_f32);
	let gizmo_pov = get_flag("DebugGizmoPoV").unwrap_or(Isometry3::identity());

	match pos {
		DebugPosition::Screen(_) => gizmo_size,
		DebugPosition::World(pos) => gizmo_size / gizmo_pov.inverse_transform_point(&pos).coords.magnitude().powf(gizmo_fallout - 1.0),
	}
}

/// Returns the `(stroke, fill)` opacity of a gizmo based on its selection state, or `None`
/// if it should be hidden.
pub fn gizmo_opacity(selected: bool, hovered: bool, hide_unselected: bool) -> Option<(f32, f32)> {
	if hide_unselected && !selected && !hovered { return None; }
	
	Some(match (selected, hovered) {
		(true, true) => (0.75, 0.5),
		(true, false) => (1.0, 0.0),
		(false, true) => (0.5, 0.5),
		(false, false) => (1.0, 0.0),
	})
}

/// Splits a line at the first comma.
fn split_comma(line: &str) -> Option<(&str, &str)> {
	let mut splitter = line.splitn(2, ',');
	let first = splitter.next()?;
	let second = splitter.next()?;
	Some((first, second))
}

lazy_static! {
	/// Japanese to English translations of MMD bone and morph names, loaded from `translations.txt`.
	static ref TRANSLATIONS: HashMap<&'static str, &'static str> = include_str!("./translations.txt").lines().filter_map(split_comma).collect();
}

/// Translates a Japanese MMD name to English, if a translation is known.
pub fn translate(text: &str) -> Option<&'static str> {
	TRANSLATIONS.get(text)
	            .cloned() // Remove double ref
}
