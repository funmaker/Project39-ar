use std::cell::{Cell, RefCell};
use std::fmt::Display;
use std::hash::Hash;
use std::num::NonZero;
use std::ops::RangeInclusive;
use egui::*;

mod application;
mod math;
mod physics;

pub fn id_fmt(id: impl Display, prefix: impl Display) -> RichText {
	RichText::new(format!("{}{:04}", prefix, id)).monospace()
}

// Hack but will do until egui supports hovering over grid
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

pub trait ExUi {
	fn inspect<T>(&mut self, value: T, options: T::Options<'_>) where T: Inspect;
	fn inspect_row<T>(&mut self, label: impl Into<WidgetText>, value: T, options: T::Options<'_>) where T: Inspect;
	fn inspect_collapsing(&mut self) -> InspectCollapsing;
	fn highlight_indent(&mut self, level: SelectionLevel);
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
			let last_hover: &mut Id = mem.get_temp_mut_or(Id::new("LastHover"), Id::null());
			
			let hovered = *last_hover == response.id;
			if !hovered && response.hovered() {
				*last_hover = response.id;
			} else if hovered && !response.hovered() {
				*last_hover = Id::null();
			}
			
			hovered
		})
	}
}


pub trait Inspect {
	type Options<'a>;
	
	fn inspect_ui(self, ui: &mut Ui, options: Self::Options<'_>);
}


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


pub trait SimpleInspect {
	fn inspect_ui(&mut self, ui: &mut Ui);
}

impl<T: SimpleInspect> InspectMut for T {
	type Options<'a> = ();
	
	fn inspect_ui(&mut self, ui: &mut Ui, _options: Self::Options<'_>) {
		SimpleInspect::inspect_ui(self, ui);
	}
}


pub trait InspectObject: Inspect + Sized {
	fn inspect_header(&self, options: &Self::Options<'_>) -> WidgetText;
	fn inspect_uid(&self, options: &Self::Options<'_>) -> u64;
	
	fn selection_level(&self, _options: &Self::Options<'_>) -> SelectionLevel {
		SelectionLevel::default()
	}
	
	fn show_collapsing(self, options: Self::Options<'_>, ui: &mut Ui, collapsing: InspectCollapsing) {
		let selection_level = self.selection_level(&options);
		ui.highlight_indent(selection_level);
		
		if selection_level.focus {
			ui.scroll_to_cursor(Some(Align::Min));
		}
		
		CollapsingHeader::new(collapsing.title.unwrap_or_else(|| self.inspect_header(&options)))
			.id_source(self.inspect_uid(&options))
			.default_open(collapsing.default_open)
			.open(selection_level.select_within.then_some(true))
			.show(ui, |ui| {
				ui.reset_style();
				
				ui.inspect(self, options);
			});
		
		ui.reset_style();
	}
}

#[derive(Debug, Copy, Clone, Default)]
pub struct SelectionLevel {
	pub hovered: bool,
	pub selected: bool,
	pub hover_within: bool,
	pub select_within: bool,
	pub focus: bool,
}

impl SelectionLevel {
	pub fn new() -> Self {
		Self::default()
	}
	
	pub fn selected() -> Self {
		SelectionLevel {
			hovered: false,
			selected: false,
			hover_within: true,
			select_within: true,
			focus: false,
		}
	}
	
	pub fn any(&self) -> bool {
		self.hovered || self.select_within || self.focus
	}
}

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
	
	pub fn title(mut self, text: impl Into<WidgetText>) -> Self {
		self.title = Some(text.into());
		self
	}
	
	pub fn maybe_title(mut self, text: Option<impl Into<WidgetText>>) -> Self {
		self.title = text.map(Into::into);
		self
	}
	
	pub fn default_open(mut self, default_open: bool) -> Self {
		self.default_open = default_open;
		self
	}
	
	pub fn show<T>(self, ui: &mut Ui, value: T, options: T::Options<'_>) where T: InspectObject {
		value.show_collapsing(options, ui, self);
	}
}


impl InspectMut for f32 {
	type Options<'a> = (f32, RangeInclusive<f32>);
	
	fn inspect_ui(&mut self, ui: &mut Ui, (speed, range): Self::Options<'_>) {
		ui.add(DragValue::new(self).speed(speed).clamp_range(range));
	}
}

impl InspectMut for f64 {
	type Options<'a> = (f64, RangeInclusive<f64>);
	
	fn inspect_ui(&mut self, ui: &mut Ui, (speed, range): Self::Options<'_>) {
		ui.add(DragValue::new(self).speed(speed).clamp_range(range));
	}
}

macro_rules! num_impl {
	( $(
		$type:ty
	),* ) => { $(
		impl InspectMut for $type {
			type Options<'a> = RangeInclusive<$type>;
			
			fn inspect_ui(&mut self, ui: &mut Ui, range: Self::Options<'_>) {
				ui.add(DragValue::new(self).clamp_range(range));
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
