use egui::*;

use crate::application::{Application, Entity, EntityRef, Hand};
use crate::component::{Component, ComponentRef};
use super::*;


impl SimpleInspect for Hand {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		use egui::*;
		
		ComboBox::from_id_source("Hand")
			.selected_text(match self {
				Hand::Right => "Right",
				Hand::Left => "Left",
			})
			.show_ui(ui, |ui| {
				ui.selectable_value(self, Hand::Right, "Right");
				ui.selectable_value(self, Hand::Left, "Left");
			});
	}
}

impl Inspect for &Entity {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(self, ui: &mut Ui, application: Self::Options<'_>) {
		self.on_gui(ui, application);
	}
}

impl InspectObject for &Entity {
	fn is_selected(&self, application: &Self::Options<'_>) -> bool {
		application.get_selection().entity() == self.as_ref()
	}
	
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		(&self.name).into()
	}
	
	fn inspect_uid(&self, _options: &Self::Options<'_>) -> u64 {
		self.id
	}
}

impl Inspect for &EntityRef {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(self, ui: &mut Ui, application: &Application) {
		if let Some(entity) = self.get(application) {
			if ui.button(id_fmt(entity.id, "")).clicked() {
				application.select(self);
			}
		} else {
			ui.label(RichText::new("NULL").monospace().italics());
		}
	}
}

impl InspectObject for &EntityRef {
	fn is_selected(&self, application: &Self::Options<'_>) -> bool {
		application.get_selection().entity() == **self
	}
	
	fn inspect_header(&self, application: &Self::Options<'_>) -> WidgetText {
		if let Some(entity) = self.get(application) {
			(&entity.name).into()
		} else {
			RichText::new("NULL").monospace().italics().into()
		}
	}
	
	fn inspect_uid(&self, application: &Self::Options<'_>) -> u64 {
		if let Some(entity) = self.get(application) {
			entity.id
		} else {
			0
		}
	}
	
	fn show_collapsing(self, application: Self::Options<'_>, ui: &mut Ui, collapsing: InspectCollapsing) {
		if let Some(entity) = self.get(application) {
			entity.show_collapsing(application, ui, collapsing);
		} else {
			Grid::new(self.inspect_uid(&application))
				.min_col_width(100.0)
				.num_columns(2)
				.show(ui, |ui| {
					ui.label(collapsing.title.unwrap_or_else(|| self.inspect_header(&application)));
					ui.label(RichText::new("NULL").monospace().italics());
					ui.end_row();
				});
		}
	}
}

impl<C: Component + ?Sized> Inspect for &C {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(self, ui: &mut Ui, application: Self::Options<'_>) {
		self.on_gui(self.entity(application), ui, application);
	}
}

impl<C: Component + ?Sized> InspectObject for &C {
	fn is_selected(&self, application: &Self::Options<'_>) -> bool {
		application.get_selection().component() == self.as_cref_dyn()
	}
	
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		self.name().into()
	}
	
	fn inspect_uid(&self, _options: &Self::Options<'_>) -> u64 {
		self.id()
	}
}

impl<C: Component + ?Sized> Inspect for &ComponentRef<C> {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(self, ui: &mut Ui, application: &Application) {
		if let Some(component) = self.get_dyn(application) {
			if ui.button(id_fmt(component.id(), "")).clicked() {
				application.select(self);
			}
		} else {
			ui.label(RichText::new("NULL").monospace().italics());
		}
	}
}

impl<C: Component + ?Sized> InspectObject for &ComponentRef<C> {
	fn is_selected(&self, application: &Self::Options<'_>) -> bool {
		application.get_selection().component() == **self
	}
	
	fn inspect_header(&self, application: &Self::Options<'_>) -> WidgetText {
		if let Some(component) = self.get_dyn(application) {
			component.name().into()
		} else {
			RichText::new("NULL").monospace().italics().into()
		}
	}
	
	fn inspect_uid(&self, application: &Self::Options<'_>) -> u64 {
		if let Some(component) = self.get_dyn(application) {
			component.id()
		} else {
			0
		}
	}
	
	fn show_collapsing(self, application: Self::Options<'_>, ui: &mut Ui, collapsing: InspectCollapsing) {
		if let Some(component) = self.get_dyn(application) {
			component.show_collapsing(application, ui, collapsing);
		} else {
			Grid::new(self.inspect_uid(&application))
				.min_col_width(100.0)
				.num_columns(2)
				.show(ui, |ui| {
					ui.label(collapsing.title.unwrap_or_else(|| self.inspect_header(&application)));
					ui.label(RichText::new("NULL").monospace().italics());
					ui.end_row();
				});
		}
	}
}
