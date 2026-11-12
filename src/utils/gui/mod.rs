use std::cell::{Cell, RefCell};
use std::fmt::Display;
use std::hash::Hash;
use std::num::NonZero;
use std::ops::RangeInclusive;
use egui::*;

mod application;
mod math;
mod physics;

/// Formats an id as zero-padded monospace text, eg. `RB 0042`.
pub fn id_fmt(id: impl Display, prefix: impl Display) -> RichText {
	RichText::new(format!("{}{:04}", prefix, id)).monospace()
}

// Hack but will do until egui supports hovering over grid
/// Ends a grid row and returns a click response covering the whole row.
pub fn end_row_interact(ui: &mut Ui, id: impl Hash) -> Response {
	let rect = Rect::from_min_max(
		[
			ui.min_rect().min.x + 16.0,
			ui.cursor().min.y,
		].into(),
		[
			ui.cursor().min.x,
			ui.min_rect().max.y,
		].into()
	);
	
	ui.end_row();
	
	ui.interact(rect, Id::new(id), Sense::click())
}

/// Inspector helpers for `egui::Ui`.
pub trait ExUi {
	/// Shows an inspector widget for `value`.
	fn inspect<T>(&mut self, value: T, options: T::Options<'_>) where T: Inspect;
	/// Shows a labeled inspector row in a grid.
	fn inspect_row<T>(&mut self, label: impl Into<WidgetText>, value: T, options: T::Options<'_>) where T: Inspect;
	/// Starts building a collapsible inspector section, shown with `InspectCollapsing::show`.
	fn inspect_collapsing(&mut self) -> InspectCollapsing;
	/// Colors the indent line of the following collapsing headers according to the selection state.
	fn highlight_indent(&mut self, level: SelectionLevel);
	/// Returns whether `response` was hovered in the previous frame and records its current
	/// hover state. Used to only update the GUI hover selection when the hover changes.
	fn track_hover(&mut self, response: &Response) -> bool;
}

impl ExUi for Ui {
	fn inspect<T>(&mut self, value: T, options: T::Options<'_>) where T: Inspect {
		value.inspect_ui(self, options);
	}
	
	fn inspect_row<T>(&mut self, label: impl Into<WidgetText>, value: T, options: T::Options<'_>) where T: Inspect {
		self.label(label);
		value.inspect_ui(self, options);
		self.end_row();
	}
	
	fn inspect_collapsing(&mut self) -> InspectCollapsing {
		InspectCollapsing::new()
	}
	
	fn highlight_indent(&mut self, level: SelectionLevel) {
		if !level.any() { return; }
		
		let style = self.style_mut();
		style.visuals.widgets.noninteractive.bg_stroke.color =
			match (level.select_within, level.hovered) {
				(true, true) => Color32::LIGHT_BLUE.gamma_multiply(0.7),
				(true, false) => Color32::LIGHT_BLUE,
				(false, true) => style.visuals.widgets.noninteractive.bg_stroke.color.gamma_multiply(0.7),
				(false, false) => return,
			};
	}
	
	fn track_hover(&mut self, response: &Response) -> bool {
		self.data_mut(|mem| {
			let last_hover: &mut Id = mem.get_temp_mut_or(Id::new("LastHover"), Id::NULL);
			
			let hovered = *last_hover == response.id;
			if !hovered && response.hovered() {
				*last_hover = response.id;
			} else if hovered && !response.hovered() {
				*last_hover = Id::NULL;
			}
			
			hovered
		})
	}
}


/// Value that can be shown in the inspector. Usually implemented for references (`&T`/`&mut T`),
/// with `Options` carrying extra parameters such as drag speed or the application.
pub trait Inspect {
	type Options<'a>;
	
	fn inspect_ui(self, ui: &mut Ui, options: Self::Options<'_>);
}


/// Mutable inspector. `&mut T` implements `Inspect` for every `T: InspectMut`.
pub trait InspectMut {
	type Options<'a>;
	
	fn inspect_ui(&mut self, ui: &mut Ui, options: Self::Options<'_>);
}

impl<T: InspectMut + ?Sized> Inspect for &mut T {
	type Options<'a> = T::Options<'a>;
	
	fn inspect_ui(self, ui: &mut Ui, options: Self::Options<'_>) {
		InspectMut::inspect_ui(self, ui, options);
	}
}


/// Mutable inspector without options.
pub trait SimpleInspect {
	fn inspect_ui(&mut self, ui: &mut Ui);
}

impl<T: SimpleInspect> InspectMut for T {
	type Options<'a> = ();
	
	fn inspect_ui(&mut self, ui: &mut Ui, _options: Self::Options<'_>) {
		SimpleInspect::inspect_ui(self, ui);
	}
}


/// Value that can be shown as its own collapsible section in the inspector.
pub trait InspectObject: Inspect + Sized {
	/// Header text of the section.
	fn inspect_header(&self, options: &Self::Options<'_>) -> WidgetText;
	/// Unique id used to persist the open state of the section.
	fn inspect_uid(&self, options: &Self::Options<'_>) -> u64;
	
	/// Selection state used to highlight the section and open it when selected.
	fn selection_level(&self, _options: &Self::Options<'_>) -> SelectionLevel {
		SelectionLevel::default()
	}
	
	/// Shows the value in a collapsing header.
	fn show_collapsing(self, options: Self::Options<'_>, ui: &mut Ui, collapsing: InspectCollapsing) {
		let selection_level = self.selection_level(&options);
		ui.highlight_indent(selection_level);
		
		if selection_level.focus {
			ui.scroll_to_cursor(Some(Align::Min));
		}
		
		CollapsingHeader::new(collapsing.title.unwrap_or_else(|| self.inspect_header(&options)))
			.id_salt(self.inspect_uid(&options))
			.default_open(collapsing.default_open)
			.open(selection_level.select_within.then_some(true))
			.show(ui, |ui| {
				ui.reset_style();
				
				ui.inspect(self, options);
			});
		
		ui.reset_style();
	}
}

/// How an object relates to the current GUI selection. See `Application::gui_selections_test`.
#[derive(Debug, Copy, Clone, Default)]
pub struct SelectionLevel {
	/// The object itself is hovered.
	pub hovered: bool,
	/// The object itself is selected.
	pub selected: bool,
	/// The object or something it owns is hovered.
	pub hover_within: bool,
	/// The object or something it owns is selected.
	pub select_within: bool,
	/// The object is selected and should be scrolled into view.
	pub focus: bool,
}

impl SelectionLevel {
	pub fn new() -> Self {
		Self::default()
	}
	
	/// Selection state used to highlight a section containing the selection.
	pub fn selected() -> Self {
		SelectionLevel {
			hovered: false,
			selected: false,
			hover_within: true,
			select_within: true,
			focus: false,
		}
	}
	
	/// Returns `true` if the object should be highlighted.
	pub fn any(&self) -> bool {
		self.hovered || self.select_within || self.focus
	}
}

/// Builder for a collapsible inspector section.
pub struct InspectCollapsing {
	title: Option<WidgetText>,
	default_open: bool,
}

impl InspectCollapsing {
	pub fn new() -> Self {
		Self {
			title: None,
			default_open: false,
		}
	}
	
	/// Overrides the section title. Defaults to `InspectObject::inspect_header`.
	pub fn title(mut self, text: impl Into<WidgetText>) -> Self {
		self.title = Some(text.into());
		self
	}
	
	/// Overrides the section title if `text` is `Some`.
	pub fn maybe_title(mut self, text: Option<impl Into<WidgetText>>) -> Self {
		self.title = text.map(Into::into);
		self
	}
	
	pub fn default_open(mut self, default_open: bool) -> Self {
		self.default_open = default_open;
		self
	}
	
	/// Shows `value` in the section.
	pub fn show<T>(self, ui: &mut Ui, value: T, options: T::Options<'_>) where T: InspectObject {
		value.show_collapsing(options, ui, self);
	}
}


impl InspectMut for f32 {
	type Options<'a> = (f32, RangeInclusive<f32>);
	
	fn inspect_ui(&mut self, ui: &mut Ui, (speed, range): Self::Options<'_>) {
		ui.add(DragValue::new(self).speed(speed).range(range));
	}
}

impl InspectMut for f64 {
	type Options<'a> = (f64, RangeInclusive<f64>);
	
	fn inspect_ui(&mut self, ui: &mut Ui, (speed, range): Self::Options<'_>) {
		ui.add(DragValue::new(self).speed(speed).range(range));
	}
}

macro_rules! num_impl {
	( $(
		$type:ty
	),* ) => { $(
		impl InspectMut for $type {
			type Options<'a> = RangeInclusive<$type>;
			
			fn inspect_ui(&mut self, ui: &mut Ui, range: Self::Options<'_>) {
				ui.add(DragValue::new(self).range(range));
			}
		}
		
		impl InspectMut for NonZero<$type> {
			type Options<'a> = RangeInclusive<$type>;
			
			fn inspect_ui(&mut self, ui: &mut Ui, range: Self::Options<'_>) {
				let mut value = self.get();
				
				ui.inspect(&mut value, range);
				
				if let Some(new_value) = Self::new(value) {
					*self = new_value;
				}
			}
		}
	)*}
}

num_impl!(i8, u8, i16, u16, i32, u32, i64, u64, isize, usize);


impl Inspect for &str {
	type Options<'a> = ();
	
	fn inspect_ui(self, ui: &mut Ui, _: Self::Options<'_>) {
		ui.label(self);
	}
}

impl Inspect for &String {
	type Options<'a> = ();
	
	fn inspect_ui(self, ui: &mut Ui, _: Self::Options<'_>) {
		ui.label(self);
	}
}

impl Inspect for String {
	type Options<'a> = ();
	
	fn inspect_ui(self, ui: &mut Ui, _: Self::Options<'_>) {
		ui.label(self);
	}
}

impl Inspect for RichText {
	type Options<'a> = ();
	
	fn inspect_ui(self, ui: &mut Ui, _: Self::Options<'_>) {
		ui.label(self);
	}
}

impl Inspect for openvr::TrackedDeviceIndex {
	type Options<'a> = ();
	
	fn inspect_ui(self, ui: &mut Ui, _: Self::Options<'_>) {
		ui.label(format!("{}", self.0));
	}
}

impl<T: InspectMut> InspectMut for Option<T> {
	type Options<'a> = T::Options<'a>;
	
	fn inspect_ui(&mut self, ui: &mut Ui, options: Self::Options<'_>) {
		if let Some(inner) = self {
			ui.inspect(inner, options)
		} else {
			ui.label(RichText::new("NONE").monospace().italics());
		}
	}
}

impl<T: InspectMut + Copy + PartialEq> Inspect for &Cell<T> {
	type Options<'a> = T::Options<'a>;
	
	fn inspect_ui(self, ui: &mut Ui, options: T::Options<'_>) {
		let mut value = self.get();
		
		ui.inspect(&mut value, options);
		
		self.set(value);
	}
}

impl<T: InspectMut + PartialEq + ?Sized> Inspect for &RefCell<T> {
	type Options<'a> = T::Options<'a>;
	
	fn inspect_ui(self, ui: &mut Ui, options: T::Options<'_>) {
		ui.inspect(&mut *self.borrow_mut(), options);
	}
}

/// Inspects a value through a getter and setter instead of a reference. The closure returns the
/// current value and a setter, which is called only if the value was changed.
pub struct GetSet<T, G, S>(pub G)
where G: FnOnce() -> (T, S);

impl<T, G, S> Inspect for GetSet<T, G, S>
	where T: InspectMut + Clone + PartialEq,
	      G: FnOnce() -> (T, S),
	      S: FnOnce(T) {
	type Options<'a> = T::Options<'a>;
	
	fn inspect_ui(self, ui: &mut Ui, options: T::Options<'_>) {
		let (org, set) = (self.0)();
		let mut value = org.clone();
		
		ui.inspect(&mut value, options);
		
		if value != org {
			(set)(value);
		}
	}
}
