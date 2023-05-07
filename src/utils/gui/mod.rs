use std::cell::{Cell, RefCell};
use std::fmt::Display;
use std::ops::RangeInclusive;
use std::sync::Arc;
use arc_swap::ArcSwapOption;
use egui::*;

mod application;
mod math;
mod physics;


pub fn id_fmt(id: impl Display, prefix: impl Display) -> RichText {
	RichText::new(format!("{}{:04}", prefix, id)).monospace()
}

pub trait ExUi {
	fn inspect<T>(&mut self, value: T, options: T::Options<'_>) where T: Inspect;
	fn inspect_row<T>(&mut self, label: impl Into<WidgetText>, value: T, options: T::Options<'_>) where T: Inspect;
	fn inspect_collapsing(&mut self) -> InspectCollapsing;
	fn highlight_indent(&mut self);
}

static HIGHLIGHT_STYLE: ArcSwapOption<Style> = ArcSwapOption::const_empty();

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
	
	fn highlight_indent(&mut self) {
		if let Some(style) = HIGHLIGHT_STYLE.load_full() {
			self.set_style(style);
		} else {
			let mut style = (**self.style()).clone();
			style.visuals.widgets.noninteractive.bg_stroke.color = Color32::LIGHT_BLUE;
			
			let style = Arc::new(style);
			HIGHLIGHT_STYLE.store(Some(style.clone()));
			self.set_style(style);
		}
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
	fn is_selected(&self, options: &Self::Options<'_>) -> bool;
	fn inspect_header(&self, options: &Self::Options<'_>) -> WidgetText;
	fn inspect_uid(&self, options: &Self::Options<'_>) -> u64;
	
	fn selection_scroll(&self, ui: &mut Ui, options: &Self::Options<'_>) {
		let self_id = self.inspect_uid(options);
		let last_scroll = ui.data_mut(|d| {
			std::mem::replace(d.get_persisted_mut_or_default(Id::new("LastScroll")), Some(self_id))
		});
		
		if last_scroll != Some(self_id) {
			ui.scroll_to_cursor(Some(Align::Min));
		}
	}
	
	fn show_collapsing(self, options: Self::Options<'_>, ui: &mut Ui, collapsing: InspectCollapsing) {
		let selected = self.is_selected(&options);
		
		if selected {
			ui.highlight_indent();
			self.selection_scroll(ui, &options);
		}
		
		CollapsingHeader::new(collapsing.title.unwrap_or_else(|| self.inspect_header(&options)))
			.id_source(self.inspect_uid(&options))
			.default_open(collapsing.default_open)
			.open(selected.then_some(true))
			.show(ui, |ui| {
				ui.reset_style();
				
				ui.inspect(self, options);
			});
		
		ui.reset_style();
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
